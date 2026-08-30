use proc_macro::{Delimiter, TokenStream, TokenTree};

#[proc_macro_attribute]
pub fn e2e(attributes: TokenStream, item: TokenStream) -> TokenStream {
    let function_name = match function_name(&item) {
        Ok(name) => name,
        Err(error) => return compile_error(error),
    };
    let metadata = match parse_metadata(attributes) {
        Ok(metadata) => metadata,
        Err(error) => return compile_error(error),
    };
    let tags = metadata.tags.join(", ");
    let generated = format!(
        "{} pub static SCENARIO: Scenario = Scenario {{ story: {}, name: {:?}, scope: {}, priority: {}, description: {}, tags: &[{}], run: {} }};",
        item,
        metadata.story,
        function_name,
        metadata.scope,
        metadata.priority,
        metadata.description,
        tags,
        function_name,
    );
    generated
        .parse()
        .unwrap_or_else(|_| compile_error("could not generate e2e scenario metadata".to_owned()))
}

struct Metadata {
    story: String,
    scope: String,
    priority: String,
    description: String,
    tags: Vec<String>,
}

fn function_name(item: &TokenStream) -> Result<String, String> {
    let mut saw_fn = false;
    for token in item.clone() {
        if saw_fn {
            return match token {
                TokenTree::Ident(name) => Ok(name.to_string()),
                _ => Err("e2e attribute must be applied to a named function".to_owned()),
            };
        }
        if matches!(&token, TokenTree::Ident(name) if name.to_string() == "fn") {
            saw_fn = true;
        }
    }
    Err("e2e attribute must be applied to a function".to_owned())
}

fn parse_metadata(attributes: TokenStream) -> Result<Metadata, String> {
    let mut tokens = attributes.into_iter().peekable();
    let mut story = None;
    let mut scope = None;
    let mut priority = None;
    let mut description = None;
    let mut tags = Vec::new();

    while let Some(token) = tokens.next() {
        let key = match token {
            TokenTree::Ident(key) => key.to_string(),
            _ => return Err("expected an e2e metadata field name".to_owned()),
        };
        expect_punctuation(&mut tokens, '=')?;
        let value = tokens
            .next()
            .ok_or_else(|| format!("missing value for e2e field `{key}`"))?;
        match key.as_str() {
            "story" => story = Some(string_literal(value, "story")?),
            "scope" => scope = Some(string_literal(value, "scope")?),
            "priority" => priority = Some(string_literal(value, "priority")?),
            "description" => description = Some(string_literal(value, "description")?),
            "tags" => tags = tag_literals(value)?,
            _ => return Err(format!("unknown e2e metadata field `{key}`")),
        }
        if tokens.peek().is_some() {
            expect_punctuation(&mut tokens, ',')?;
        }
    }

    Ok(Metadata {
        story: story.ok_or_else(|| "e2e metadata requires `story`".to_owned())?,
        scope: scope.ok_or_else(|| "e2e metadata requires `scope`".to_owned())?,
        priority: priority.ok_or_else(|| "e2e metadata requires `priority`".to_owned())?,
        description: description.ok_or_else(|| "e2e metadata requires `description`".to_owned())?,
        tags,
    })
}

fn expect_punctuation<I>(tokens: &mut I, expected: char) -> Result<(), String>
where
    I: Iterator<Item = TokenTree>,
{
    match tokens.next() {
        Some(TokenTree::Punct(punctuation)) if punctuation.as_char() == expected => Ok(()),
        _ => Err(format!("expected `{expected}` in e2e metadata")),
    }
}

fn string_literal(value: TokenTree, field: &str) -> Result<String, String> {
    match value {
        TokenTree::Literal(value)
            if value.to_string().starts_with('"') || value.to_string().starts_with('r') =>
        {
            Ok(value.to_string())
        }
        _ => Err(format!("e2e field `{field}` must be a string literal")),
    }
}

fn tag_literals(value: TokenTree) -> Result<Vec<String>, String> {
    let TokenTree::Group(group) = value else {
        return Err("e2e field `tags` must be an array".to_owned());
    };
    if group.delimiter() != Delimiter::Bracket {
        return Err("e2e field `tags` must use square brackets".to_owned());
    }
    let mut tags = Vec::new();
    for token in group.stream() {
        match token {
            TokenTree::Literal(tag)
                if tag.to_string().starts_with('"') || tag.to_string().starts_with('r') =>
            {
                tags.push(tag.to_string());
            }
            TokenTree::Punct(punctuation) if punctuation.as_char() == ',' => {}
            _ => return Err("e2e tags must contain only string literals".to_owned()),
        }
    }
    Ok(tags)
}

fn compile_error(message: String) -> TokenStream {
    format!("compile_error!({message:?});")
        .parse()
        .expect("valid compile error")
}
