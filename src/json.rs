use std::collections::HashMap;

use anyhow::{anyhow, Result};
use winnow::{
    ascii::{digit1, multispace0, Caseless},
    combinator::{alt, delimited, opt, separated, separated_pair, trace},
    error::{ContextError, ErrMode, ParserError},
    stream::{AsBStr, AsChar, Compare, FindSlice, ParseSlice, Stream, StreamIsPartial},
    token::take_until,
    PResult, Parser,
};

#[derive(Debug, Clone, PartialEq)]
enum Num {
    Int(i64),
    Float(f64),
}

#[allow(unused)]
#[derive(Debug, Clone, PartialEq)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(Num),
    String(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

fn main() -> Result<()> {
    let s = r#"{
        "name": "John Doe",
        "age": 30,
        "is_student": false,
        "marks": [90.0, 80, 85],
        "address": {
            "city": "New York",
            "zip": 10001
        }
    }"#;
    let input = &mut s.as_bytes();
    let v = parse_json(input)?;
    println!("{:#?}", v);
    Ok(())
}

fn parse_json(input: &[u8]) -> Result<JsonValue> {
    let input = &mut (&*input);
    parse_value(input).map_err(|e: ErrMode<ContextError>| anyhow!("Failed to parse JSON: {:?}", e))
}

pub fn sep_with_space<Input, Output, Error, ParseNext>(
    mut parser: ParseNext,
) -> impl Parser<Input, (), Error>
where
    Input: Stream + StreamIsPartial,
    <Input as Stream>::Token: AsChar + Clone,
    Error: ParserError<Input>,
    ParseNext: Parser<Input, Output, Error>,
{
    trace("sep_with_space", move |input: &mut Input| {
        let _ = multispace0.parse_next(input)?;
        parser.parse_next(input)?;
        multispace0.parse_next(input)?;
        Ok(())
    })
}

pub fn parse_null<Input, Error>(input: &mut Input) -> PResult<(), Error>
where
    Input: StreamIsPartial + Stream + Compare<&'static str>,
    Error: ParserError<Input>,
{
    "null".value(()).parse_next(input)
}

fn parse_bool<Input, Error>(input: &mut Input) -> PResult<bool, Error>
where
    Input: StreamIsPartial + Stream + Compare<&'static str>,
    <Input as Stream>::Slice: ParseSlice<bool>,
    Error: ParserError<Input>,
{
    alt(("true", "false")).parse_to().parse_next(input)
}

fn parse_num<Input, Error>(input: &mut Input) -> PResult<Num, Error>
where
    Input: StreamIsPartial
        + Stream
        + Compare<&'static str>
        + Compare<Caseless<&'static str>>
        + Compare<char>
        + AsBStr,
    <Input as Stream>::Token: AsChar + Clone,
    <Input as Stream>::Slice: ParseSlice<i64> + ParseSlice<f64>,
    <Input as Stream>::IterOffsets: Clone,
    Error: ParserError<Input>,
{
    let sign = opt("-").map(|s| s.is_some()).parse_next(input)?;
    let num = digit1.parse_to::<i64>().parse_next(input)?;
    let ret: Result<_, ErrMode<ContextError>> = ".".value(()).parse_next(input);
    if ret.is_ok() {
        let frac = digit1.parse_to::<i64>().parse_next(input)?;
        let v = format!("{}.{}", num, frac).parse::<f64>().unwrap();

        Ok(if sign { Num::Float(-v) } else { Num::Float(v) })
    } else {
        Ok(if sign { Num::Int(-num) } else { Num::Int(num) })
    }
}

// fn parse_number<Input, Error>(input: &mut Input) -> PResult<f64, Error>
// where
//     Input: StreamIsPartial + Stream + Compare<Caseless<&'static str>> + Compare<char> + AsBStr,
//     <Input as Stream>::Slice: ParseSlice<f64>,
//     <Input as Stream>::Token: AsChar + Clone,
//     <Input as Stream>::IterOffsets: Clone,
//     Error: ParserError<Input>,
// {
//     float.parse_next(input)
// }

fn parse_string<Input, Error>(input: &mut Input) -> PResult<String, Error>
where
    Input: StreamIsPartial + Stream + Compare<char> + FindSlice<char>,
    <Input as Stream>::Token: AsChar,
    <Input as Stream>::Slice: ParseSlice<String>,
    Error: ParserError<Input>,
{
    delimited('"', take_until(0.., '"').parse_to::<String>(), '"').parse_next(input)
}

fn parse_array<Input, Error>(input: &mut Input) -> PResult<Vec<JsonValue>, Error>
where
    Input: StreamIsPartial
        + Stream
        + Compare<&'static str>
        + Compare<char>
        + Compare<Caseless<&'static str>>
        + AsBStr
        + FindSlice<char>,
    <Input as Stream>::Token: AsChar,
    <Input as Stream>::Slice:
        ParseSlice<bool> + ParseSlice<i64> + ParseSlice<f64> + ParseSlice<String>,
    <Input as Stream>::Token: AsChar + Clone,
    <Input as Stream>::IterOffsets: Clone,
    Error: ParserError<Input>,
{
    let sep1 = sep_with_space('[');
    let sep2 = sep_with_space(']');
    let sep_comma = sep_with_space(',');

    let parse_values = separated(0.., parse_value, sep_comma);
    delimited(sep1, parse_values, sep2).parse_next(input)
}

fn parse_object<Input, Error>(input: &mut Input) -> PResult<HashMap<String, JsonValue>, Error>
where
    Input: StreamIsPartial
        + Stream
        + Compare<&'static str>
        + Compare<char>
        + Compare<Caseless<&'static str>>
        + AsBStr
        + FindSlice<char>,
    <Input as Stream>::Token: AsChar,
    <Input as Stream>::Slice:
        ParseSlice<bool> + ParseSlice<i64> + ParseSlice<f64> + ParseSlice<String>,
    <Input as Stream>::Token: AsChar + Clone,
    <Input as Stream>::IterOffsets: Clone,
    Error: ParserError<Input>,
{
    let sep1 = sep_with_space('{');
    let sep2 = sep_with_space('}');
    let sep_comma = sep_with_space(',');
    let sep_colon = sep_with_space(':');
    let parse_kv_pair = separated_pair(parse_string, sep_colon, parse_value);
    let parse_kv = separated(1.., parse_kv_pair, sep_comma);
    delimited(sep1, parse_kv, sep2).parse_next(input)
}

fn parse_value<Input, Error>(input: &mut Input) -> PResult<JsonValue, Error>
where
    Input: StreamIsPartial
        + Stream
        + Compare<&'static str>
        + Compare<Caseless<&'static str>>
        + Compare<char>
        + AsBStr
        + FindSlice<char>,
    <Input as Stream>::Slice:
        ParseSlice<bool> + ParseSlice<i64> + ParseSlice<f64> + ParseSlice<String>,
    <Input as Stream>::Token: AsChar + Clone,
    <Input as Stream>::IterOffsets: Clone,
    Error: ParserError<Input>,
{
    alt((
        parse_null.value(JsonValue::Null),
        parse_bool.map(JsonValue::Bool),
        parse_num.map(JsonValue::Number),
        parse_string.map(JsonValue::String),
        parse_array.map(JsonValue::Array),
        parse_object.map(JsonValue::Object),
    ))
    .parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_null() -> PResult<(), ContextError> {
        let input = "null";
        parse_null(&mut (&*input))?;
        Ok(())
    }

    #[test]
    fn test_parse_bool() -> PResult<(), ContextError> {
        let input = "true";
        let v = parse_bool(&mut (&*input))?;
        assert!(v);
        let input = "false";
        let v = parse_bool(&mut (&*input))?;
        assert!(!v);
        Ok(())
    }

    #[test]
    fn test_parse_num() -> PResult<(), ContextError> {
        let input = "123";
        let v = parse_num(&mut (&*input))?;
        assert_eq!(v, Num::Int(123));

        let input = "-123";
        let v = parse_num(&mut (&*input))?;
        assert_eq!(v, Num::Int(-123));

        let input = "123.456";
        let v = parse_num(&mut (&*input))?;
        assert_eq!(v, Num::Float(123.456));

        let input = "-123.456";
        let v = parse_num(&mut (&*input))?;
        assert_eq!(v, Num::Float(-123.456));
        Ok(())
    }

    #[test]
    fn test_parse_string() -> PResult<(), ContextError> {
        let input = r#""hello""#;
        let v = parse_string(&mut (&*input))?;
        assert_eq!(v, "hello");
        Ok(())
    }

    #[test]
    fn test_parse_array() -> PResult<(), ContextError> {
        let input = r#"[1,2,3]"#;
        let v = parse_array(&mut (&*input))?;
        assert_eq!(
            v,
            vec![
                JsonValue::Number(Num::Int(1)),
                JsonValue::Number(Num::Int(2)),
                JsonValue::Number(Num::Int(3))
            ]
        );

        let input = r#"["a","b","c"]"#;
        let v = parse_array(&mut (&*input))?;
        assert_eq!(
            v,
            vec![
                JsonValue::String("a".to_string()),
                JsonValue::String("b".to_string()),
                JsonValue::String("c".to_string()),
            ]
        );
        Ok(())
    }

    #[test]
    fn test_parse_object() -> PResult<(), ContextError> {
        let input = r#"{"a":1,"b":"hello"}"#;
        let v = parse_object(&mut (&*input))?;
        assert_eq!(
            v,
            HashMap::from([
                ("a".to_string(), JsonValue::Number(Num::Int(1))),
                ("b".to_string(), JsonValue::String("hello".to_string()))
            ])
        );
        Ok(())
    }
}
