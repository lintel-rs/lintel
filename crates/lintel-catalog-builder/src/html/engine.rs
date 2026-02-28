use anyhow::{Context, Result};
use minijinja::{AutoEscape, Environment};

/// Create a [`minijinja::Environment`] with all templates registered.
pub fn create_engine() -> Result<Environment<'static>> {
    let mut env = Environment::new();
    env.set_trim_blocks(true);
    env.set_lstrip_blocks(true);

    // Override the default formatter to use HTML escaping that does NOT
    // escape `/`. The default minijinja HTML escaper converts `/` to
    // `&#x2f;` (OWASP recommendation), but our data is trusted (from config
    // files), and the escaped slashes make URLs and file paths ugly.
    env.set_formatter(|out, state, value| {
        use minijinja::value::ValueKind;
        // Safe strings (via |safe filter) must be written verbatim.
        if value.is_safe() {
            return write!(out, "{value}").map_err(Into::into);
        }
        if state.auto_escape() == AutoEscape::None {
            write!(out, "{value}").map_err(Into::into)
        } else {
            let s = if let Some(s) = value.as_str() {
                html_escape(s)
            } else if matches!(
                value.kind(),
                ValueKind::Undefined | ValueKind::None | ValueKind::Bool | ValueKind::Number
            ) {
                return write!(out, "{value}").map_err(Into::into);
            } else {
                html_escape(&value.to_string())
            };
            out.write_str(&s).map_err(Into::into)
        }
    });

    env.add_filter("commafy", commafy_filter);
    env.add_filter("pluralize", pluralize_filter);

    register_templates(&mut env)?;

    Ok(env)
}

/// Extract a `usize` from a minijinja [`Value`](minijinja::Value).
fn value_as_usize(value: &minijinja::Value) -> usize {
    value.as_usize().unwrap_or_else(|| {
        #[allow(clippy::cast_sign_loss)]
        i64::try_from(value.clone())
            .ok()
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(0)
    })
}

/// Filter: format a number with comma separators (e.g. `1234` → `"1,234"`).
#[allow(clippy::needless_pass_by_value)]
fn commafy_filter(value: minijinja::Value) -> String {
    let n = value_as_usize(&value);
    let s = alloc::format!("{n}");
    let mut result = alloc::string::String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Filter: choose singular or plural form based on a count.
///
/// Usage: `{{ count|pluralize("schema", "schemas") }}`
#[allow(clippy::needless_pass_by_value)]
fn pluralize_filter(
    value: minijinja::Value,
    singular: alloc::string::String,
    plural: alloc::string::String,
) -> String {
    if value_as_usize(&value) == 1 {
        singular
    } else {
        plural
    }
}

/// HTML-escape a string, escaping `& < > "` but not `/`.
fn html_escape(value: &str) -> alloc::string::String {
    let mut out = alloc::string::String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn register_templates(env: &mut Environment<'static>) -> Result<()> {
    let templates: &[(&str, &str)] = &[
        ("layout.html", include_str!("templates/layout.html")),
        ("index.html", include_str!("templates/index.html")),
        ("group.html", include_str!("templates/group.html")),
        ("schema.html", include_str!("templates/schema.html")),
        ("version.html", include_str!("templates/version.html")),
        ("shared.html", include_str!("templates/shared.html")),
        ("sitemap.xml", include_str!("templates/sitemap.xml")),
        (
            "components/schema_card.html",
            include_str!("templates/components/schema_card.html"),
        ),
        (
            "components/group_card.html",
            include_str!("templates/components/group_card.html"),
        ),
        (
            "components/breadcrumb.html",
            include_str!("templates/components/breadcrumb.html"),
        ),
        (
            "components/search_bar.html",
            include_str!("templates/components/search_bar.html"),
        ),
        (
            "components/theme_toggle.html",
            include_str!("templates/components/theme_toggle.html"),
        ),
        (
            "components/schema_doc.html",
            include_str!("templates/components/schema_doc.html"),
        ),
        (
            "components/property_tree.html",
            include_str!("templates/components/property_tree.html"),
        ),
    ];

    for (name, source) in templates {
        env.add_template(name, source)
            .with_context(|| alloc::format!("failed to register template '{name}'"))?;
    }

    Ok(())
}

/// Render a named template with the given serializable context.
pub fn render<S: serde::Serialize>(
    env: &Environment<'_>,
    template_name: &str,
    ctx: &S,
) -> Result<String> {
    let tmpl = env
        .get_template(template_name)
        .with_context(|| alloc::format!("template '{template_name}' not found"))?;
    tmpl.render(ctx)
        .with_context(|| alloc::format!("failed to render template '{template_name}'"))
}
