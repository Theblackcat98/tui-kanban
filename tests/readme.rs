//! The README's keyboard reference is generated from the command table.
//! Regenerate it with `UPDATE_README=1 cargo test --test readme`.

use std::fs;
use std::path::Path;

const START: &str = "<!-- keys:start -->\n";
const END: &str = "<!-- keys:end -->";

#[test]
fn the_readme_lists_the_keys_in_the_command_table() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    // Git may check the file out with Windows line endings.
    let readme = fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
    let start = readme.find(START).expect("the README has a keys section") + START.len();
    let end = readme[start..].find(END).expect("the keys section ends") + start;
    let expected = format!("\n{}\n", tui_kanban::command::markdown_reference());
    if readme[start..end] != expected && std::env::var_os("UPDATE_README").is_some() {
        let updated = format!("{}{expected}{}", &readme[..start], &readme[end..]);
        fs::write(&path, updated).unwrap();
        return;
    }
    assert!(
        readme[start..end] == expected,
        "the README's keys are out of date: run UPDATE_README=1 cargo test --test readme"
    );
}
