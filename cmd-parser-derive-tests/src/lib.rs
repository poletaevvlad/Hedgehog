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
