#[cfg(test)]

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
}
