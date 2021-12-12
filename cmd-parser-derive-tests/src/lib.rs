// #![cfg(test)]

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
        );
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
        );
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
        assert_eq!(Enum::parse_cmd("unit def").unwrap(), (Enum::Unit, "def"));
    }

    #[test]
    fn tuple() {
        assert_eq!(
            Enum::parse_cmd("tuple 10 20 def").unwrap(),
            (Enum::Tuple(10, 20), "def")
        );
    }

    #[test]
    fn struct_variant() {
        assert_eq!(
            Enum::parse_cmd("struct 10 20 def").unwrap(),
            (Enum::Struct { a: 10, b: 20 }, "def")
        );
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
        assert_eq!(Enum::parse_cmd("fifth").unwrap().0, Enum::Fifth);
        assert_eq!(Enum::parse_cmd("5").unwrap().0, Enum::Fifth);
        assert_eq!(Enum::parse_cmd("a").unwrap().0, Enum::Nested1(Nested1::A));
        assert_eq!(Enum::parse_cmd("b").unwrap().0, Enum::Nested1(Nested1::B));
        assert_eq!(Enum::parse_cmd("c").unwrap().0, Enum::Nested1(Nested1::C));
        assert_eq!(Enum::parse_cmd("d").unwrap().0, Enum::Nested2(Nested2::D));
        assert_eq!(Enum::parse_cmd("e").unwrap().0, Enum::Nested2(Nested2::E));
    }

    #[test]
    fn cannot_parse() {
        Enum::parse_cmd("first").unwrap_err();
        Enum::parse_cmd("second").unwrap_err();
        Enum::parse_cmd("fourth").unwrap_err();
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

mod named_attribure {
    use cmd_parser::CmdParsable;

    #[derive(Debug, PartialEq, CmdParsable)]
    enum Enum {
        Tuple(u8, #[cmd(attr(second_attr, two = "2"))] u8),
        Struct {
            a: u8,
            #[cmd(attr(second_attr, two = "2"))]
            b: u8,
        },
        TupleAllOptional(#[cmd(attr(a = "true"))] bool, #[cmd(attr(b = "true"))] bool),
    }

    #[test]
    fn using_default() {
        assert_eq!(Enum::parse_cmd_full("tuple 5").unwrap(), Enum::Tuple(5, 0));
        assert_eq!(
            Enum::parse_cmd_full("struct 5").unwrap(),
            Enum::Struct { a: 5, b: 0 }
        );
    }

    #[test]
    fn setting_named() {
        assert_eq!(
            Enum::parse_cmd_full("tuple 5 --second-attr 3").unwrap(),
            Enum::Tuple(5, 3)
        );
        assert_eq!(
            Enum::parse_cmd_full("struct 5 --second-attr 3").unwrap(),
            Enum::Struct { a: 5, b: 3 }
        );
    }

    #[test]
    fn setting_predefined() {
        assert_eq!(
            Enum::parse_cmd_full("tuple 5 --two").unwrap(),
            Enum::Tuple(5, 2)
        );
        assert_eq!(
            Enum::parse_cmd_full("struct 5 --two").unwrap(),
            Enum::Struct { a: 5, b: 2 }
        );
    }

    #[test]
    fn unknown_attribute() {
        assert_eq!(
            Enum::parse_cmd_full("tuple 2 --abc")
                .unwrap_err()
                .to_string(),
            "unknown attribute: \"--abc\""
        );
        assert_eq!(
            Enum::parse_cmd_full("struct 2 --abc")
                .unwrap_err()
                .to_string(),
            "unknown attribute: \"--abc\""
        );
    }

    #[test]
    fn all_optional() {
        assert_eq!(
            Enum::parse_cmd_full("tuple-all-optional").unwrap(),
            Enum::TupleAllOptional(false, false)
        );
        assert_eq!(
            Enum::parse_cmd_full("tuple-all-optional --a").unwrap(),
            Enum::TupleAllOptional(true, false)
        );
        assert_eq!(
            Enum::parse_cmd_full("tuple-all-optional --b").unwrap(),
            Enum::TupleAllOptional(false, true)
        );
        assert_eq!(
            Enum::parse_cmd_full("tuple-all-optional --b --a").unwrap(),
            Enum::TupleAllOptional(true, true)
        );
    }

    #[test]
    fn stops_after_last_required() {
        assert_eq!(
            <(Enum, Vec<u8>)>::parse_cmd_full("tuple 10 20 30").unwrap(),
            (Enum::Tuple(10, 0), vec![20, 30])
        );
    }

    #[test]
    fn missing_required() {
        assert_eq!(
            &Enum::parse_cmd_full("tuple").unwrap_err().to_string(),
            "expected integer"
        );
    }
}

mod alias_overrides {
    use cmd_parser::CmdParsable;

    #[derive(Debug, PartialEq, Eq, CmdParsable)]
    enum Enum {
        #[cmd(alias = "one", alias = "two")]
        AllOverriden(#[cmd(alias_override(one = "1", two = "2"))] u8),

        #[cmd(alias = "three", alias = "four")]
        SomeOverriden(#[cmd(alias_override(three = "3", four = "4"))] u8, u8),

        #[cmd(alias = "five")]
        Struct {
            #[cmd(alias_override(five = "5"))]
            val: u8,
        },
    }

    #[test]
    fn all_overriden() {
        assert_eq!(Enum::parse_cmd("one").unwrap().0, Enum::AllOverriden(1));
        assert_eq!(Enum::parse_cmd("two").unwrap().0, Enum::AllOverriden(2));
        assert_eq!(
            Enum::parse_cmd("all-overriden 3").unwrap().0,
            Enum::AllOverriden(3)
        );
    }

    #[test]
    fn some_overriden() {
        assert_eq!(
            Enum::parse_cmd("three 10").unwrap().0,
            Enum::SomeOverriden(3, 10)
        );
        assert_eq!(
            Enum::parse_cmd("four 11").unwrap().0,
            Enum::SomeOverriden(4, 11)
        );
        assert_eq!(
            Enum::parse_cmd("some-overriden 5 12").unwrap().0,
            Enum::SomeOverriden(5, 12)
        );
    }

    #[test]
    fn parse_struct() {
        assert_eq!(Enum::parse_cmd("five").unwrap().0, Enum::Struct { val: 5 });
    }
}
