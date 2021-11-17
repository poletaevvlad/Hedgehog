#![cfg(test)]

mod simple_struct {
    use cmd_parser::CmdParsable;

    #[test]
    fn unit_struct() {
        #[derive(Debug, PartialEq, CmdParsable)]
        struct UnitStruct;

        let result = UnitStruct::parse_cmd("abc").unwrap();
        assert_eq!(result, (UnitStruct, "abc"));
    }

    #[test]
    fn struct_unnamed() {
        #[derive(Debug, PartialEq, CmdParsable)]
        struct Struct(u8, String);

        assert_eq!(
            Struct::parse_cmd("10 abc def").unwrap(),
            (Struct(10, "abc".to_string()), "def")
        )
    }

    #[test]
    fn struct_named() {
        #[derive(Debug, PartialEq, CmdParsable)]
        struct Struct {
            int: u8,
            text: String,
        }

        assert_eq!(
            Struct::parse_cmd("10 abc def").unwrap(),
            (
                Struct {
                    int: 10,
                    text: "abc".to_string()
                },
                "def"
            )
        )
    }

    #[test]
    fn struct_generic_type() {
        #[derive(Debug, PartialEq, CmdParsable)]
        struct Struct {
            a: u8,
            b: Vec<u8>,
        }

        assert_eq!(
            Struct::parse_cmd("10 20 30 40").unwrap(),
            (
                Struct {
                    a: 10,
                    b: vec![20, 30, 40]
                },
                ""
            )
        );
    }

    #[test]
    fn tuple_generic_type() {
        #[derive(Debug, PartialEq, CmdParsable)]
        struct Struct(u8, Vec<u8>);

        assert_eq!(
            Struct::parse_cmd("10 20 30 40").unwrap(),
            (Struct(10, vec![20, 30, 40]), "")
        );
    }
}

mod simple_enum {
    use cmd_parser::CmdParsable;

    #[derive(Debug, PartialEq, CmdParsable)]
    enum Enum {
        Unit,
        Tuple(u8, u32),
        Struct { a: u8, b: u32 },
    }

    #[test]
    fn union() {
        assert_eq!(Enum::parse_cmd("Unit def").unwrap(), (Enum::Unit, "def"))
    }

    #[test]
    fn tuple() {
        assert_eq!(
            Enum::parse_cmd("Tuple 10 20 def").unwrap(),
            (Enum::Tuple(10, 20), "def")
        )
    }

    #[test]
    fn struct_variant() {
        assert_eq!(
            Enum::parse_cmd("Struct 10 20 def").unwrap(),
            (Enum::Struct { a: 10, b: 20 }, "def")
        )
    }
}

mod enum_attributes {
    use cmd_parser::CmdParsable;

    #[derive(Debug, PartialEq, CmdParsable)]
    enum Enum {
        #[cmd(rename = "f")]
        First,
        #[cmd(ignore, alias = "s2", alias = "second2")]
        Second,
        #[cmd(rename = "t")]
        #[cmd(alias = "t1")]
        #[cmd(alias = "t2")]
        Third,
        #[cmd(ignore)]
        #[allow(dead_code)]
        Fourth,
        #[cmd(alias = "5")]
        Fifth,
        #[cmd(transparent)]
        Nested1(Nested1),
        #[cmd(transparent)]
        Nested2(Nested2),
    }

    #[derive(Debug, PartialEq, CmdParsable)]
    enum Nested1 {
        A,
        B,
        C,
    }

    #[derive(Debug, PartialEq, CmdParsable)]
    enum Nested2 {
        D,
        E,
    }

    #[test]
    fn can_parse() {
        assert_eq!(Enum::parse_cmd("f").unwrap().0, Enum::First);
        assert_eq!(Enum::parse_cmd("s2").unwrap().0, Enum::Second);
        assert_eq!(Enum::parse_cmd("second2").unwrap().0, Enum::Second);
        assert_eq!(Enum::parse_cmd("t").unwrap().0, Enum::Third);
        assert_eq!(Enum::parse_cmd("t1").unwrap().0, Enum::Third);
        assert_eq!(Enum::parse_cmd("t2").unwrap().0, Enum::Third);
        assert_eq!(Enum::parse_cmd("Fifth").unwrap().0, Enum::Fifth);
        assert_eq!(Enum::parse_cmd("5").unwrap().0, Enum::Fifth);
        assert_eq!(Enum::parse_cmd("A").unwrap().0, Enum::Nested1(Nested1::A));
        assert_eq!(Enum::parse_cmd("B").unwrap().0, Enum::Nested1(Nested1::B));
        assert_eq!(Enum::parse_cmd("C").unwrap().0, Enum::Nested1(Nested1::C));
        assert_eq!(Enum::parse_cmd("D").unwrap().0, Enum::Nested2(Nested2::D));
        assert_eq!(Enum::parse_cmd("E").unwrap().0, Enum::Nested2(Nested2::E));
    }

    #[test]
    fn cannot_parse() {
        Enum::parse_cmd("First").unwrap_err();
        Enum::parse_cmd("Second").unwrap_err();
        Enum::parse_cmd("Fourth").unwrap_err();
    }
}

mod custom_parser {
    use cmd_parser::{CmdParsable, ParseError};

    fn mock_parser(input: &str) -> Result<(u8, &str), ParseError> {
        let mut chars = input.chars();
        let ch = chars.next().unwrap().to_string().parse().unwrap();
        Ok((ch, chars.as_str()))
    }

    #[test]
    fn parse_struct() {
        #[derive(Debug, PartialEq, CmdParsable)]
        struct Struct {
            #[cmd(parse_with = "mock_parser")]
            a: u8,
            b: u8,
        }
        assert_eq!(Struct::parse_cmd("12").unwrap().0, Struct { a: 1, b: 2 });
    }

    #[test]
    fn parse_tuple_struct() {
        #[derive(Debug, PartialEq, CmdParsable)]
        struct Struct(#[cmd(parse_with = "mock_parser")] u8, u8);
        assert_eq!(Struct::parse_cmd("12").unwrap().0, Struct(1, 2));
    }
}
