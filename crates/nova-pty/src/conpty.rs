//! Windows pseudoconsole implementation.

use std::ffi::c_void;
use std::io::{self, Read, Write};

use windows::core::{Error as WinError, HRESULT, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, ERROR_INSUFFICIENT_BUFFER, FALSE, HANDLE, INVALID_HANDLE_VALUE,
    WAIT_OBJECT_0,
};
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, INFINITE,
    LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, STARTUPINFOEXW, STARTUPINFOW,
};

use crate::{CommandBuilder, PtyError, PtySize, Result};

/// The undocumented-but-stable attribute id selecting a pseudoconsole for a
/// child process. (`ProcThreadAttributeValue(22, FALSE, TRUE, FALSE)`.)
const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x0002_0016;

/// A spawned pseudoconsole and its child process.
///
/// `Pty` owns the pseudoconsole handle and the child process handle. The two
/// pipe ends used for I/O are handed out exactly once each via
/// [`Pty::take_reader`] / [`Pty::take_writer`] so they can be moved onto
/// dedicated I/O threads.
pub struct Pty {
    hpc: HPCON,
    child: HANDLE,
    reader: Option<PtyReader>,
    writer: Option<PtyWriter>,
}

// SAFETY: the contained handles are owned exclusively by this `Pty` and are
// only used through `&self`/`&mut self`; moving ownership across threads is safe.
unsafe impl Send for Pty {}

impl Pty {
    /// Spawn `command` inside a fresh pseudoconsole of `size`.
    pub fn spawn(command: &CommandBuilder, size: PtySize) -> Result<Pty> {
        unsafe { Self::spawn_inner(command, size) }
    }

    unsafe fn spawn_inner(command: &CommandBuilder, size: PtySize) -> Result<Pty> {
        // --- 1. Create the two pipes. ----------------------------------------
        // input:  we write -> child reads      (pty_in_read goes to the console)
        // output: child writes -> we read      (pty_out_write goes to the console)
        let mut input_read = INVALID_HANDLE_VALUE;
        let mut input_write = INVALID_HANDLE_VALUE;
        let mut output_read = INVALID_HANDLE_VALUE;
        let mut output_write = INVALID_HANDLE_VALUE;

        CreatePipe(&mut input_read, &mut input_write, None, 0)
            .map_err(|e| PtyError::CreatePipe(e.message()))?;
        CreatePipe(&mut output_read, &mut output_write, None, 0).map_err(|e| {
            let _ = CloseHandle(input_read);
            let _ = CloseHandle(input_write);
            PtyError::CreatePipe(e.message())
        })?;

        // --- 2. Create the pseudoconsole. ------------------------------------
        let coord = COORD {
            X: size.cols as i16,
            Y: size.rows as i16,
        };
        let hpc = match CreatePseudoConsole(coord, input_read, output_write, 0) {
            Ok(h) => h,
            Err(e) => {
                for h in [input_read, input_write, output_read, output_write] {
                    let _ = CloseHandle(h);
                }
                return Err(PtyError::CreatePseudoConsole(e.message()));
            }
        };

        // The console duplicated the ends it needs; close our copies.
        let _ = CloseHandle(input_read);
        let _ = CloseHandle(output_write);

        // --- 3. Build STARTUPINFOEX with the pseudoconsole attribute. --------
        let mut siex = STARTUPINFOEXW::default();
        siex.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;

        // Determine the attribute-list size, then allocate it.
        let mut attr_size: usize = 0;
        let _ = InitializeProcThreadAttributeList(
            LPPROC_THREAD_ATTRIBUTE_LIST(std::ptr::null_mut()),
            1,
            0,
            &mut attr_size,
        );
        let mut attr_buf = vec![0u8; attr_size];
        let attr_list = LPPROC_THREAD_ATTRIBUTE_LIST(attr_buf.as_mut_ptr() as *mut c_void);

        if let Err(e) = InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_size) {
            ClosePseudoConsole(hpc);
            let _ = CloseHandle(input_write);
            let _ = CloseHandle(output_read);
            return Err(PtyError::Spawn {
                program: command.command_line(),
                reason: e.message(),
            });
        }

        let update = UpdateProcThreadAttribute(
            attr_list,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
            Some(hpc.0 as *const c_void),
            std::mem::size_of::<HPCON>(),
            None,
            None,
        );
        if let Err(e) = update {
            DeleteProcThreadAttributeList(attr_list);
            ClosePseudoConsole(hpc);
            let _ = CloseHandle(input_write);
            let _ = CloseHandle(output_read);
            return Err(PtyError::Spawn {
                program: command.command_line(),
                reason: e.message(),
            });
        }
        siex.lpAttributeList = attr_list;

        // --- 4. Spawn the child. ---------------------------------------------
        let mut cmdline = to_wide(&command.command_line());
        let cwd_wide = command.cwd_ref().map(to_wide);
        let env_block = build_env_block(command);

        // Match the reference (EchoCon) flags: only request a Unicode
        // environment when we actually supply one.
        let mut flags = EXTENDED_STARTUPINFO_PRESENT;
        if env_block.is_some() {
            flags |= CREATE_UNICODE_ENVIRONMENT;
        }

        let mut pi = PROCESS_INFORMATION::default();
        let create = CreateProcessW(
            PCWSTR::null(),
            PWSTR(cmdline.as_mut_ptr()),
            None,
            None,
            FALSE,
            flags,
            env_block.as_ref().map(|b| b.as_ptr() as *const c_void),
            cwd_wide
                .as_ref()
                .map_or(PCWSTR::null(), |w| PCWSTR(w.as_ptr())),
            &siex.StartupInfo as *const STARTUPINFOW,
            &mut pi,
        );

        // Attribute list and the console's pipe copies are no longer needed here.
        DeleteProcThreadAttributeList(attr_list);

        if let Err(e) = create {
            ClosePseudoConsole(hpc);
            let _ = CloseHandle(input_write);
            let _ = CloseHandle(output_read);
            return Err(PtyError::Spawn {
                program: command.command_line(),
                reason: e.message(),
            });
        }

        // We don't need the primary thread handle.
        let _ = CloseHandle(pi.hThread);

        Ok(Pty {
            hpc,
            child: pi.hProcess,
            reader: Some(PtyReader {
                handle: output_read,
            }),
            writer: Some(PtyWriter {
                handle: input_write,
            }),
        })
    }

    /// Take the output reader (the child's stdout/stderr stream). Returns `None`
    /// if it was already taken.
    pub fn take_reader(&mut self) -> Option<PtyReader> {
        self.reader.take()
    }

    /// Take the input writer (the child's stdin stream).
    pub fn take_writer(&mut self) -> Option<PtyWriter> {
        self.writer.take()
    }

    /// Resize the pseudoconsole. Safe to call while the child is running.
    pub fn resize(&self, size: PtySize) -> Result<()> {
        let coord = COORD {
            X: size.cols as i16,
            Y: size.rows as i16,
        };
        unsafe { ResizePseudoConsole(self.hpc, coord) }.map_err(|e| PtyError::Resize(e.message()))
    }

    /// Block until the child exits, returning its exit code.
    pub fn wait(&self) -> Result<i32> {
        unsafe {
            if WaitForSingleObject(self.child, INFINITE) != WAIT_OBJECT_0 {
                return Err(PtyError::Io(io::Error::last_os_error()));
            }
            self.exit_code()
        }
    }

    /// Return the exit code if the child has exited, else `None`.
    pub fn try_wait(&self) -> Result<Option<i32>> {
        unsafe {
            if WaitForSingleObject(self.child, 0) == WAIT_OBJECT_0 {
                Ok(Some(self.exit_code()?))
            } else {
                Ok(None)
            }
        }
    }

    unsafe fn exit_code(&self) -> Result<i32> {
        let mut code: u32 = 0;
        GetExitCodeProcess(self.child, &mut code).map_err(|e| PtyError::Io(win_io(e)))?;
        Ok(code as i32)
    }

    /// Forcibly terminate the child process.
    pub fn kill(&self) -> Result<()> {
        unsafe { TerminateProcess(self.child, 1) }.map_err(|e| PtyError::Io(win_io(e)))
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        unsafe {
            // Closing the pseudoconsole signals EOF to the child and lets it exit.
            ClosePseudoConsole(self.hpc);
            let _ = CloseHandle(self.child);
        }
    }
}

/// The readable end of the pseudoconsole output. Implements [`std::io::Read`]
/// via blocking `ReadFile`; a broken pipe (child exit) reports as EOF.
pub struct PtyReader {
    handle: HANDLE,
}

// SAFETY: exclusive ownership of the handle; only used from one thread at a time.
unsafe impl Send for PtyReader {}

impl Read for PtyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut read: u32 = 0;
        let res = unsafe { ReadFile(self.handle, Some(buf), Some(&mut read), None) };
        match res {
            Ok(()) => Ok(read as usize),
            Err(e) if is_broken_pipe(&e) => Ok(0),
            Err(e) => Err(win_io(e)),
        }
    }
}

impl Drop for PtyReader {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// The writable end of the pseudoconsole input. Implements [`std::io::Write`].
pub struct PtyWriter {
    handle: HANDLE,
}

// SAFETY: see `PtyReader`.
unsafe impl Send for PtyWriter {}

impl Write for PtyWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut written: u32 = 0;
        let res = unsafe { WriteFile(self.handle, Some(buf), Some(&mut written), None) };
        match res {
            Ok(()) => Ok(written as usize),
            Err(e) => Err(win_io(e)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for PtyWriter {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

fn is_broken_pipe(e: &WinError) -> bool {
    e.code() == HRESULT::from_win32(ERROR_BROKEN_PIPE.0)
        || e.code() == HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0)
}

fn win_io(e: WinError) -> io::Error {
    io::Error::other(e.message())
}

/// UTF-16, null-terminated.
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Build a `CREATE_UNICODE_ENVIRONMENT` block, or `None` to inherit the parent
/// environment verbatim.
fn build_env_block(cmd: &CommandBuilder) -> Option<Vec<u16>> {
    let custom: Vec<_> = cmd.env_iter().collect();
    if cmd.inherits_env() && custom.is_empty() {
        return None; // inherit as-is
    }

    let mut merged: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    if cmd.inherits_env() {
        for (k, v) in std::env::vars() {
            merged.insert(k, v);
        }
    }
    for (k, v) in custom {
        merged.insert(k.clone(), v.clone());
    }

    let mut block: Vec<u16> = Vec::new();
    for (k, v) in merged {
        block.extend(format!("{k}={v}").encode_utf16());
        block.push(0);
    }
    block.push(0); // double-null terminator
    Some(block)
}
