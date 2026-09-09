use fp_core::intrinsics::{
    CallKind, LangIntrinsic, LangIntrinsicCapability, StdIntrinsic, lang_intrinsic_call_kind,
    lang_intrinsic_capability, lang_intrinsic_for_lang_item, lang_intrinsic_lang_item,
};
use std::collections::HashSet;
#[test]
fn std_intrinsic_variants_are_hashable() {
    let mut set = HashSet::new();
    set.insert(StdIntrinsic::IoPrint);
    set.insert(StdIntrinsic::IoPrintln);
    assert_eq!(set.len(), 2);
    assert!(set.contains(&StdIntrinsic::IoPrint));
}

#[test]
fn intrinsic_call_kind_matches_expected_variants() {
    assert_eq!(CallKind::Print, CallKind::Print);
    assert_ne!(CallKind::Print, CallKind::Println);
}

#[test]
fn lang_intrinsic_maps_time_lang_item_to_call_kind() {
    let intrinsic = lang_intrinsic_for_lang_item("time_now").expect("missing lang intrinsic");
    assert_eq!(intrinsic, LangIntrinsic::TimeNow);
    assert_eq!(lang_intrinsic_call_kind(intrinsic), Some(CallKind::TimeNow));
    assert_eq!(
        lang_intrinsic_capability(intrinsic),
        LangIntrinsicCapability::RuntimeOnly
    );
}

#[test]
fn lang_intrinsic_preserves_fs_lang_item_name() {
    let intrinsic =
        lang_intrinsic_for_lang_item("fs_read_to_string").expect("missing fs lang intrinsic");
    assert_eq!(intrinsic, LangIntrinsic::FsReadToString);
    assert_eq!(lang_intrinsic_lang_item(intrinsic), "fs_read_to_string");
    assert_eq!(
        lang_intrinsic_call_kind(intrinsic),
        Some(CallKind::FsReadToString)
    );
}

#[test]
fn lang_intrinsic_maps_core_fs_lang_items_to_call_kinds() {
    let cases = [
        (
            "fs_write_string",
            LangIntrinsic::FsWriteString,
            CallKind::FsWriteString,
        ),
        (
            "fs_append_string",
            LangIntrinsic::FsAppendString,
            CallKind::FsAppendString,
        ),
        ("fs_exists", LangIntrinsic::FsExists, CallKind::FsExists),
        ("fs_is_dir", LangIntrinsic::FsIsDir, CallKind::FsIsDir),
        ("fs_is_file", LangIntrinsic::FsIsFile, CallKind::FsIsFile),
    ];

    for (lang_item, expected_intrinsic, expected_call_kind) in cases {
        let intrinsic = lang_intrinsic_for_lang_item(lang_item).expect("missing fs lang intrinsic");
        assert_eq!(intrinsic, expected_intrinsic);
        assert_eq!(lang_intrinsic_lang_item(intrinsic), lang_item);
        assert_eq!(
            lang_intrinsic_call_kind(intrinsic),
            Some(expected_call_kind)
        );
    }
}
