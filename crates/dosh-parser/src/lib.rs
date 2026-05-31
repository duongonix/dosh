mod command;
mod expr;
mod ident;
mod parser;
mod syntax;
mod types;

pub use expr::parse_expression_result;
pub use parser::Parser;

#[cfg(test)]
mod tests {
    use super::Parser;
    use dosh_ast::{Expression, Statement};

    #[test]
    fn parse_empty_line() {
        let parser = Parser::new();
        let script = parser.parse_line("   ").unwrap();
        assert!(script.statements.is_empty());
    }

    #[test]
    fn parse_simple_command_with_args() {
        let parser = Parser::new();
        let script = parser.parse_line("echo hello world").unwrap();

        match &script.statements[0] {
            Statement::Command(cmd) => {
                assert_eq!(cmd.name, "echo");
                assert_eq!(cmd.args, vec!["hello", "world"]);
                assert!(!cmd.background);
                assert!(!cmd.force_external);
            }
            _ => panic!("expected command"),
        }
    }

    #[test]
    fn parse_assignment_statement() {
        let parser = Parser::new();
        let script = parser.parse_line("$count = 42").unwrap();
        match &script.statements[0] {
            Statement::Assignment(assign) => {
                let name = &assign.name;
                let value = &assign.value;
                assert_eq!(name, "count");
                assert!(matches!(value, Expression::Integer(42)));
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_pipeline_statement() {
        let parser = Parser::new();
        let script = parser.parse_line("echo hello | findstr h").unwrap();
        match &script.statements[0] {
            Statement::Pipeline(p) => assert_eq!(p.commands.len(), 2),
            _ => panic!("expected pipeline"),
        }
    }

    #[test]
    fn parse_where_comparison_in_pipeline() {
        let parser = Parser::new();
        let script = parser
            .parse_line("ls | where size > 1mb | sort-by modified")
            .unwrap();
        match &script.statements[0] {
            Statement::Pipeline(p) => {
                assert_eq!(p.commands.len(), 3);
                assert_eq!(p.commands[1].name, "where");
                assert_eq!(p.commands[1].args, vec!["size", ">", "1mb"]);
                assert!(p.commands[1].redirects.is_empty());
            }
            _ => panic!("expected pipeline"),
        }
    }

    #[test]
    fn parse_redirect_and_background() {
        let parser = Parser::new();
        let script = parser.parse_line("echo ok > out.txt &").unwrap();
        match &script.statements[0] {
            Statement::Command(cmd) => {
                assert!(cmd.background);
                assert_eq!(cmd.redirects.len(), 1);
            }
            _ => panic!("expected command"),
        }
    }

    #[test]
    fn parse_function_for_match_and_import() {
        let parser = Parser::new();
        let source = r#"
            fn greet($name) { echo hi }
            module util { fn foo() { echo ok } }
            use util
            for $item in "a b" { greet($item) }
            match 1 { 1 => { echo one }; _ => { echo two } }
        "#;

        let script = parser.parse_script(source).unwrap();
        assert_eq!(script.statements.len(), 5);
        assert!(matches!(script.statements[0], Statement::Function { .. }));
        assert!(matches!(script.statements[1], Statement::Module { .. }));
        assert!(matches!(script.statements[2], Statement::Import { .. }));
        assert!(matches!(script.statements[3], Statement::For { .. }));
        assert!(matches!(script.statements[4], Statement::Match { .. }));
    }

    #[test]
    fn parse_mod_alias_for_module_statement() {
        let parser = Parser::new();
        let source = r#"
            mod util { fn foo() { echo ok } }
            use util
        "#;
        let script = parser.parse_script(source).unwrap();
        assert_eq!(script.statements.len(), 2);
        assert!(matches!(script.statements[0], Statement::Module { .. }));
        assert!(matches!(script.statements[1], Statement::Import { .. }));
    }

    #[test]
    fn parse_dollar_assignment_and_constant() {
        let parser = Parser::new();
        let script = parser
            .parse_script("$name = \"hello\"\n$NAME = \"DoSH\"")
            .unwrap();
        assert_eq!(script.statements.len(), 2);
        match &script.statements[0] {
            Statement::Assignment(assign) => {
                assert_eq!(assign.name, "name");
                assert!(!assign.is_constant);
            }
            _ => panic!("expected assignment"),
        }
        match &script.statements[1] {
            Statement::Assignment(assign) => {
                assert_eq!(assign.name, "NAME");
                assert!(assign.is_constant);
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_cell_path_variable() {
        let parser = Parser::new();
        let script = parser.parse_line("print $user.profile.email").unwrap();
        assert_eq!(script.statements.len(), 1);
    }

    #[test]
    fn parse_nested_assignment_target() {
        let parser = Parser::new();
        let script = parser.parse_line("$user.name = \"x\"").unwrap();
        match &script.statements[0] {
            Statement::Assignment(assign) => assert_eq!(assign.cell_path.len(), 1),
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_print_interpolation_token_keeps_var() {
        let parser = Parser::new();
        let script = parser.parse_line("print \"loop n=$n\"").unwrap();
        match &script.statements[0] {
            Statement::Command(cmd) => assert_eq!(cmd.args, vec!["loop n=$n"]),
            _ => panic!("expected command"),
        }
    }

    #[test]
    fn parse_caret_external_command() {
        let parser = Parser::new();
        let script = parser
            .parse_line("^\"C:\\Program Files\\exiftool.exe\" -ver")
            .unwrap();
        match &script.statements[0] {
            Statement::Command(cmd) => {
                assert!(cmd.force_external);
                assert_eq!(cmd.name, "C:\\Program Files\\exiftool.exe");
                assert_eq!(cmd.args, vec!["-ver"]);
            }
            _ => panic!("expected command"),
        }
    }

    #[test]
    fn parse_caret_variable_external_command() {
        let parser = Parser::new();
        let script = parser.parse_line("^$foo --help").unwrap();
        match &script.statements[0] {
            Statement::Command(cmd) => {
                assert!(cmd.force_external);
                assert_eq!(cmd.name, "$foo");
                assert_eq!(cmd.args, vec!["--help"]);
            }
            _ => panic!("expected command"),
        }
    }

    #[test]
    fn parse_string_literal_pipeline_source() {
        let parser = Parser::new();
        let script = parser.parse_line("\"a\" | save b.txt").unwrap();
        match &script.statements[0] {
            Statement::Pipeline(p) => {
                assert_eq!(p.commands.len(), 2);
                assert_eq!(p.commands[0].name, "__literal__");
                assert_eq!(p.commands[0].args, vec!["\"a\""]);
                assert_eq!(p.commands[1].name, "save");
            }
            _ => panic!("expected pipeline"),
        }
    }

    #[test]
    fn parse_number_pipeline_source() {
        let parser = Parser::new();
        let script = parser.parse_line("1 | str length").unwrap();
        match &script.statements[0] {
            Statement::Pipeline(p) => {
                assert_eq!(p.commands[0].name, "__literal__");
                assert_eq!(p.commands[0].args, vec!["1"]);
            }
            _ => panic!("expected pipeline"),
        }
    }

    #[test]
    fn parse_record_list_pipeline_source() {
        let parser = Parser::new();
        let script = parser.parse_line("[{a: \"b\"}] | save a.json").unwrap();
        match &script.statements[0] {
            Statement::Pipeline(p) => {
                assert_eq!(p.commands[0].name, "__literal__");
                assert_eq!(p.commands[1].name, "save");
            }
            _ => panic!("expected pipeline"),
        }
    }

    #[test]
    fn parse_list_of_records_expression_keeps_records() {
        let expr =
            crate::parse_expression_result("[{name:\"a\", age:20},{name:\"b\", age:21}]").unwrap();
        match expr {
            Expression::List(items) => {
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], Expression::Record(_)));
                assert!(matches!(items[1], Expression::Record(_)));
            }
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn parse_record_pipeline_source() {
        let parser = Parser::new();
        let script = parser.parse_line("{name:\"dosh\"} | save d.json").unwrap();
        match &script.statements[0] {
            Statement::Pipeline(p) => {
                assert_eq!(p.commands[0].name, "__literal__");
                assert_eq!(p.commands[1].name, "save");
            }
            _ => panic!("expected pipeline"),
        }
    }

    #[test]
    fn parse_use_and_export_forms() {
        let parser = Parser::new();
        let source = r#"
            use "./utils.dosh" as utils
            export fn greet($name) { print $name }
            export $NAME = "DoSH"
        "#;
        let script = parser.parse_script(source).unwrap();
        assert!(matches!(script.statements[0], Statement::Import { .. }));
        assert!(matches!(script.statements[1], Statement::Function { .. }));
        assert!(matches!(script.statements[2], Statement::Assignment(_)));
    }

    #[test]
    fn parse_test_block() {
        let parser = Parser::new();
        let script = parser
            .parse_script("test \"add works\" { assert eq 1 1 }")
            .unwrap();
        assert!(matches!(script.statements[0], Statement::Test { .. }));
    }

    #[test]
    fn parse_if_elif_else_chain() {
        let parser = Parser::new();
        let script = parser
            .parse_script(r#"if 1 == 2 { print "a" } elif 2 == 2 { print "b" } else { print "c" }"#)
            .unwrap();
        match &script.statements[0] {
            Statement::If { else_branch, .. } => {
                assert_eq!(else_branch.len(), 1);
                assert!(matches!(else_branch[0], Statement::If { .. }));
            }
            _ => panic!("expected if"),
        }
    }
}
