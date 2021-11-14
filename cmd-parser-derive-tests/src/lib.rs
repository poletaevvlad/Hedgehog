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
}
