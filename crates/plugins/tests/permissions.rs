use caliberate_plugins::sandbox::Permission;
use caliberate_plugins::sandbox::Permissions;

#[test]
fn maps_permissions() {
    let perms = Permissions::from_strings(&vec![
        "files.read".to_string(),
        "network".to_string(),
        "device".to_string(),
    ])
    .expect("permissions");
    assert!(perms.allows_files_read);
    assert!(perms.allows_network);
    assert!(perms.allows_device);
    assert!(!perms.allows_files_write);
}

#[test]
fn parses_permission_enum() {
    let perm = Permission::parse("files.write").expect("parse");
    assert_eq!(perm, Permission::FilesWrite);
}
