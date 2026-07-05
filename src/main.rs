use cliclack::{intro, outro, outro_cancel, select};

fn main() -> std::io::Result<()> {
    intro("init-env")?;

    let Ok(tool) = select("Select a tool for your development environment")
        .item("node", "Node.js", "")
        .item("python", "Python", "")
        .item("go", "Go", "")
        .item("rust", "Rust", "")
        .interact()
    else {
        outro_cancel("Cancelled")?;
        return Ok(());
    };

    outro(format!("Selected: {tool}"))?;

    Ok(())
}
