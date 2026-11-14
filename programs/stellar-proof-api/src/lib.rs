// SPDX-License-Identifier: Apache-2.0

use anyhow::bail;

pub fn expand_environment(
    text: &str,
    lookup: impl Fn(&str) -> Option<String>,
) -> anyhow::Result<String> {
    let mut expanded = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find("${") {
        expanded.push_str(&rest[..start]);

        let after = &rest[start + 2..];

        let Some(end) = after.find('}') else {
            bail!("the configuration has an unclosed ${{ placeholder");
        };

        let name = &after[..end];

        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            bail!("the configuration placeholder ${{{name}}} is not an environment variable name");
        }

        let Some(value) = lookup(name) else {
            bail!("the configuration names ${{{name}}}, but {name} is not set");
        };

        expanded.push_str(&value);
        rest = &after[end + 1..];
    }

    expanded.push_str(rest);

    Ok(expanded)
}
