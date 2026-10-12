use sql_killer_core::{database, project};
fn main() {
    let dir = std::path::Path::new("examples");
    std::fs::create_dir_all(dir).unwrap();
    for fixture in ["student", "hotel", "bank", "empty"] {
        let mut w = project::Workspace {
            fixture: fixture.into(),
            ..Default::default()
        };
        if fixture != "student" {
            w.documents[0].text = match fixture {
                "hotel" => "SELECT * FROM Hotel;",
                "bank" => "SELECT * FROM Account;",
                _ => "CREATE TABLE Notes(Id INTEGER PRIMARY KEY, Body TEXT NOT NULL);",
            }
            .into();
        }
        project::save(
            &dir.join(format!("{fixture}.sqlkproj")),
            &w,
            &database::memory(fixture).unwrap(),
        )
        .unwrap();
    }
}
