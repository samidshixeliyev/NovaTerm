//! Wire round-trip sanity: every protocol type must survive JSON serialization.

use nova_protocol::*;

#[test]
fn color_packing() {
    let c = Color::rgb(0x12, 0x34, 0x56);
    assert_eq!((c.r(), c.g(), c.b(), c.a()), (0x12, 0x34, 0x56, 0xff));
    assert!(Color::DEFAULT.is_default());
}

#[test]
fn cell_attrs_ops() {
    let mut a = CellAttrs::empty();
    assert!(a.is_empty());
    a.insert(CellAttrs::BOLD | CellAttrs::ITALIC);
    assert!(a.contains(CellAttrs::BOLD));
    assert!(a.contains(CellAttrs::ITALIC));
    a.remove(CellAttrs::BOLD);
    assert!(!a.contains(CellAttrs::BOLD));
    assert!(a.contains(CellAttrs::ITALIC));
}

#[test]
fn frame_diff_roundtrip() {
    let diff = FrameDiff {
        session: SessionId::new(),
        seq: 7,
        cols: 80,
        rows: 24,
        full: false,
        scroll: Some(ScrollRegion {
            top: 0,
            bottom: 23,
            delta: 1,
        }),
        runs: vec![RowRun {
            row: 3,
            col: 5,
            cells: vec![Cell {
                ch: 'X',
                fg: Color::WHITE,
                bg: Color::BLACK,
                attrs: CellAttrs::BOLD,
            }],
        }],
        cursor: CursorState::default(),
        scrollback_len: 1000,
    };
    let json = serde_json::to_string(&diff).unwrap();
    let back: FrameDiff = serde_json::from_str(&json).unwrap();
    assert_eq!(diff, back);
}

#[test]
fn command_tagged_enum() {
    let cmd = Command::Input {
        session: SessionId::new(),
        event: InputEvent::Key(KeyEvent {
            key: "Enter".into(),
            mods: KeyModifiers::default(),
            text: None,
        }),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("\"cmd\":\"input\""));
    let back: Command = serde_json::from_str(&json).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn event_tagged_enum() {
    let ev = CoreEvent::Exited {
        session: SessionId::new(),
        code: 0,
    };
    let json = serde_json::to_string(&ev).unwrap();
    assert!(json.contains("\"event\":\"exited\""));
}
