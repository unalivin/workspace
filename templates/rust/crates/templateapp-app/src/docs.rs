use app_core::service;

use crate::cli;

fn replace_block(document: &str, name: &str, content: &str) -> Result<String, String> {
    let start = format!("<!-- {name}:start -->\n");
    let end = format!("<!-- {name}:end -->");
    let from = document.find(&start).ok_or_else(|| format!("docs/COMMANDS.md lacks the {name} start marker"))?;
    let to = document.find(&end).ok_or_else(|| format!("docs/COMMANDS.md lacks the {name} end marker"))?;
    if to < from {
        return Err(format!("docs/COMMANDS.md has the {name} markers in the wrong order"));
    }
    Ok(format!("{}{content}{}", &document[..from + start.len()], &document[to..]))
}

pub fn commands_document(current: &str) -> Result<String, String> {
    let with_capabilities = replace_block(current, "capabilities", &service::capability_table())?;
    replace_block(&with_capabilities, "reference", &cli::reference())
}
