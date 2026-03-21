use std::fs;
use std::path::PathBuf;

#[test]
fn assistant_message_protocol_doc_is_marked_deprecated() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .expect("openjax-protocol crate directory should have a workspace parent");
    let doc_path = workspace_root.join("docs/protocol/v1/protocol-v1.md");
    let doc = fs::read_to_string(&doc_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", doc_path.display()));

    assert!(
        doc.contains("AssistantMessage"),
        "protocol-v1 doc should still mention AssistantMessage for compatibility tracking"
    );
    assert!(
        doc.contains("deprecated"),
        "protocol-v1 doc should include the literal `deprecated` guardrail keyword"
    );
}
