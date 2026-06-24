#[cfg(test)]
mod tests {
    use crate::parse;

    #[test]
    fn test_parse_simple_workflow() {
        let input = r#"workflow Test {
  input {
    x: string
  }
  policy {
    deny fs.read(path) if not cwd.contains(path)
  }
  stage@entry Hello {
    prompt: "Hello world"
    input: World.field?
    output: {
      x: string
      ok: bool {
        true => Fin
        false => Hello
      }
    }
    requires: fs.read, user.confirm
  }
  stage@exit Fin {
    prompt: "done"
    output: {
      summary: string
    }
  }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        assert_eq!(ast.name.text, "Test");
        assert_eq!(ast.inputs.len(), 1, "should have 1 workflow input");
        assert_eq!(ast.policies.len(), 1, "should have 1 policy");
        assert_eq!(ast.stages.len(), 2, "should have 2 stages");

        let entry = &ast.stages[0];
        assert_eq!(entry.name.text, "Hello");
        assert!(
            entry
                .annotations
                .iter()
                .any(|a| matches!(a, crate::ast::StageAnnotation::Entry)),
            "Hello should be @entry"
        );
        assert_eq!(entry.items.len(), 4, "Hello should have 4 body items");

        let exit = &ast.stages[1];
        assert_eq!(exit.name.text, "Fin");
        assert!(
            exit.annotations
                .iter()
                .any(|a| matches!(a, crate::ast::StageAnnotation::Exit)),
            "Fin should be @exit"
        );
    }

    #[test]
    fn test_parse_coding_agent() {
        let input = include_str!("../tests/fixtures/coding-agent.nemo");
        let ast =
            parse::parse_source(input, "coding-agent.nemo").expect("should parse coding-agent");
        assert_eq!(ast.stages.len(), 7, "coding-agent has 7 stages");
        assert_eq!(ast.inputs.len(), 2);
        assert_eq!(ast.policies.len(), 3);
    }

    #[test]
    fn test_parse_multiline_prompt() {
        let input = r#"workflow Test {
  stage S {
    prompt: """
line 1
line 2"""
    output: { x: string }
  }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        let stage = &ast.stages[0];
        for item in &stage.items {
            if let crate::ast::StageBodyItem::Prompt(s) = item {
                assert!(
                    s.value.contains('\n'),
                    "multiline prompt should contain newline"
                );
            }
        }
    }

    #[test]
    fn test_parse_optional_input_ref() {
        let input = r#"workflow Test {
  stage S {
    prompt: "x"
    input: Other.field?
    output: { x: string }
  }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        let stage = &ast.stages[0];
        for item in &stage.items {
            if let crate::ast::StageBodyItem::Input(refs) = item {
                assert_eq!(refs.len(), 1);
                assert!(refs[0].optional, "input ref should be optional");
            }
        }
    }

    #[test]
    fn test_parse_optional_output() {
        let input = r#"workflow Test {
  stage S {
    prompt: "x"
    output: {
      val: string?
    }
  }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        let stage = &ast.stages[0];
        for item in &stage.items {
            if let crate::ast::StageBodyItem::Output(fields) = item {
                assert_eq!(fields.len(), 1);
                assert!(fields[0].ty.optional, "output type should be optional");
            }
        }
    }

    #[test]
    fn test_parse_array_type() {
        let input = r#"workflow Test {
  stage S {
    prompt: "x"
    output: {
      items: string[]
    }
  }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        let stage = &ast.stages[0];
        for item in &stage.items {
            if let crate::ast::StageBodyItem::Output(fields) = item {
                assert_eq!(fields.len(), 1);
                assert!(fields[0].ty.is_array, "output type should be array");
            }
        }
    }

    #[test]
    fn test_parse_no_annotations() {
        let input = r#"workflow Test {
  stage First {
    prompt: "first"
    output: { x: string }
  }
  stage Last {
    prompt: "last"
  }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        assert_eq!(ast.stages.len(), 2);
        assert!(
            ast.stages[0].annotations.is_empty(),
            "no annotations parsed"
        );
        assert!(
            ast.stages[1].annotations.is_empty(),
            "no annotations parsed"
        );
    }

    #[test]
    fn test_parse_policy_parens_and_or() {
        let input = r#"workflow Test {
  input { command: string }
  policy {
    deny os.shell(command) if not (command.eq("python harness/preflight.py") or command.starts_with("git commit -m "))
  }
  stage@entry S { prompt: "x" output: { x: string } }
  stage@exit F { prompt: "d" input: S.x output: { summary: string } }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        assert_eq!(ast.policies.len(), 1);
        let cond = ast.policies[0]
            .condition
            .as_ref()
            .expect("should have condition");
        // Should parse as Not(Or(eq(...), starts_with(...)))
        if let crate::ast::PolicyExpr::Not { expr } = cond {
            if let crate::ast::PolicyExpr::Or { exprs } = expr.as_ref() {
                assert_eq!(exprs.len(), 2, "should have 2 disjuncts");
            } else {
                panic!("expected Or, got {:?}", expr);
            }
        } else {
            panic!("expected Not, got {:?}", cond);
        }
    }

    #[test]
    fn test_parse_policy_in_with_ref_and_string() {
        let input = r#"workflow Test {
  input { candidate_path: path }
  policy {
    deny fs.write(path) if not path in [candidate_path, "candidate_bak.py"]
  }
  stage@entry S { prompt: "x" output: { x: string } }
  stage@exit F { prompt: "d" input: S.x output: { summary: string } }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        assert_eq!(ast.policies.len(), 1);
        let cond = ast.policies[0]
            .condition
            .as_ref()
            .expect("should have condition");
        // Should parse as Not(In(value=path, options=[candidate_path, "candidate_bak.py"]))
        if let crate::ast::PolicyExpr::Not { expr } = cond {
            if let crate::ast::PolicyExpr::In { value, options } = expr.as_ref() {
                assert_eq!(value.text, "path");
                assert_eq!(options.len(), 2);
            } else {
                panic!("expected In, got {:?}", expr);
            }
        } else {
            panic!("expected Not, got {:?}", cond);
        }
    }
}
