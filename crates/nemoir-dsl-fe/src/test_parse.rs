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

    // ---- Extension 4: numeric guards parse tests (plan §8.2) ----

    #[test]
    fn test_parse_number_literal_in_transition() {
        let input = r#"workflow Test {
  stage@entry S {
    prompt: "x"
    output: { score: number }
    transition if score > 0.5 => A
  }
  stage@exit A { prompt: "done" }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        let stage = &ast.stages[0];
        for item in &stage.items {
            if let crate::ast::StageBodyItem::Transition(trans) = item {
                assert_eq!(trans.len(), 1);
                if let Some(crate::ast::PolicyExpr::Compare { right, .. }) = &trans[0].cond {
                    if let crate::ast::PolicyExpr::Number(n) = right.as_ref() {
                        assert!((n.value - 0.5).abs() < 0.001, "expected 0.5");
                        return;
                    }
                }
                panic!("expected Compare with Number(0.5) right operand");
            }
        }
        panic!("expected Transition body item");
    }

    #[test]
    fn test_parse_binop_and_compare_precedence() {
        // a + b > c  →  Compare(gt, BinOp(add, a, b), c)
        let input = r#"workflow Test {
  input { a: number b: number c: number }
  stage@entry S {
    prompt: "x"
    output: { score: number }
    transition if a + b > c => A
  }
  stage@exit A { prompt: "done" }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        let stage = &ast.stages[0];
        for item in &stage.items {
            if let crate::ast::StageBodyItem::Transition(trans) = item {
                assert_eq!(trans.len(), 1);
                if let Some(crate::ast::PolicyExpr::Compare { op, left, .. }) = &trans[0].cond {
                    assert_eq!(op, ">");
                    if let crate::ast::PolicyExpr::BinOp { op: binop, .. } = left.as_ref() {
                        assert_eq!(binop, "+");
                        return;
                    }
                }
                panic!("expected Compare(>, BinOp(+, ..), ..)");
            }
        }
        panic!("expected Transition body item");
    }

    #[test]
    fn test_parse_unary_minus() {
        // Regression for High #1: `score > -1` must NOT drop the minus.
        // The AST represents `-1` as BinOp(sub, Number(0), Number(1)).
        let input = r#"workflow Test {
  stage@entry S {
    prompt: "x"
    output: { score: number }
    transition if score > -1 => A
  }
  stage@exit A { prompt: "done" }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        let stage = &ast.stages[0];
        for item in &stage.items {
            if let crate::ast::StageBodyItem::Transition(trans) = item {
                assert_eq!(trans.len(), 1);
                if let Some(crate::ast::PolicyExpr::Compare { right, .. }) = &trans[0].cond {
                    // After unary-minus fix, -1 → BinOp(sub, 0, 1)
                    if let crate::ast::PolicyExpr::BinOp {
                        op,
                        left,
                        right: inner,
                    } = right.as_ref()
                    {
                        assert_eq!(op, "-");
                        // left = Number(0.0), right = Number(1.0)
                        if let crate::ast::PolicyExpr::Number(z) = left.as_ref() {
                            assert!((z.value - 0.0).abs() < 0.001);
                        } else {
                            panic!("expected Number(0) as unary-minus left");
                        }
                        if let crate::ast::PolicyExpr::Number(o) = inner.as_ref() {
                            assert!((o.value - 1.0).abs() < 0.001);
                            return;
                        } else {
                            panic!("expected Number(1) as unary-minus operand");
                        }
                    }
                }
                panic!("expected Compare(>, .., BinOp(sub, 0, 1))");
            }
        }
        panic!("expected Transition body item");
    }

    #[test]
    fn test_parse_transition_declaration() {
        let input = r#"workflow Test {
  stage@entry S {
    prompt: "x"
    output: { ok: bool }
    transition if ok => A
    transition else => B
  }
  stage@exit A { prompt: "a" }
  stage@exit B { prompt: "b" }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        let stage = &ast.stages[0];
        for item in &stage.items {
            if let crate::ast::StageBodyItem::Transition(trans) = item {
                assert_eq!(trans.len(), 2);
                // First transition: if ok => A
                assert_eq!(trans[0].target.text, "A");
                assert!(matches!(
                    trans[0].cond,
                    Some(crate::ast::PolicyExpr::Ref(_))
                ));
                // Second transition: else => B
                assert_eq!(trans[1].target.text, "B");
                assert!(trans[1].cond.is_none());
                return;
            }
        }
        panic!("expected Transition body item");
    }

    #[test]
    fn test_parse_bare_ident_in_transition_condition() {
        // `transition if score => A` — bare ident `score` should parse as Ref.
        let input = r#"workflow Test {
  stage@entry S {
    prompt: "x"
    output: { score: number }
    transition if score => A
  }
  stage@exit A { prompt: "a" }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        let stage = &ast.stages[0];
        for item in &stage.items {
            if let crate::ast::StageBodyItem::Transition(trans) = item {
                assert_eq!(trans.len(), 1);
                if let Some(crate::ast::PolicyExpr::Ref(id)) = &trans[0].cond {
                    assert_eq!(id.text, "score");
                    return;
                }
                panic!("expected Ref(score)");
            }
        }
        panic!("expected Transition body item");
    }

    #[test]
    fn test_parse_bare_ref_with_keyword_prefix_in_transition() {
        let input = r#"workflow Test {
  input { index: bool notebook: bool not_flag: bool not1: bool a_input: bool }
  stage@entry A {
    prompt: "p"
    output: { ok: bool }
    transition if index => B
    transition if notebook => B
    transition if not_flag => B
    transition if not1 => B
    transition if a_input => B
    transition else => B
  }
  stage@exit B { prompt: "x" output: { done: bool } }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        let stage = &ast.stages[0];
        for item in &stage.items {
            if let crate::ast::StageBodyItem::Transition(trans) = item {
                assert_eq!(trans.len(), 6, "should have 6 transitions");
                // index reference
                if let Some(crate::ast::PolicyExpr::Ref(id)) = &trans[0].cond {
                    assert_eq!(id.text, "index");
                } else {
                    panic!("expected Ref(index)");
                }
                // notebook reference
                if let Some(crate::ast::PolicyExpr::Ref(id)) = &trans[1].cond {
                    assert_eq!(id.text, "notebook");
                } else {
                    panic!("expected Ref(notebook)");
                }
                // not_flag reference
                if let Some(crate::ast::PolicyExpr::Ref(id)) = &trans[2].cond {
                    assert_eq!(id.text, "not_flag");
                } else {
                    panic!("expected Ref(not_flag)");
                }
                // not1 reference
                if let Some(crate::ast::PolicyExpr::Ref(id)) = &trans[3].cond {
                    assert_eq!(id.text, "not1");
                } else {
                    panic!("expected Ref(not1)");
                }
                // a_input reference
                if let Some(crate::ast::PolicyExpr::Ref(id)) = &trans[4].cond {
                    assert_eq!(id.text, "a_input");
                } else {
                    panic!("expected Ref(a_input)");
                }
                // else = no condition
                assert!(trans[5].cond.is_none(), "expected None for else");
                return;
            }
        }
        panic!("expected Transition body item");
    }

    #[test]
    fn test_parse_bare_ref_with_keyword_prefix_in_policy() {
        let input = r#"workflow Test {
  input { index: bool notebook: bool not_flag: bool not1: bool cwd: path }
  policy {
    deny fs.read(path) if notebook
    deny fs.read(path) if not_flag
    deny fs.read(path) if not1
  }
  stage@entry S { prompt: "x" output: { x: string } }
  stage@exit F { prompt: "d" input: S.x output: { summary: string } }
}"#;
        let ast = parse::parse_source(input, "test.nemo").expect("parse should succeed");
        assert_eq!(ast.policies.len(), 3);
        // First policy: Ref("notebook")
        let cond = ast.policies[0]
            .condition
            .as_ref()
            .expect("should have condition");
        if let crate::ast::PolicyExpr::Ref(id) = cond {
            assert_eq!(id.text, "notebook");
        } else {
            panic!("expected Ref(notebook), got {:?}", cond);
        }
        // Second policy: Ref("not_flag")
        let cond = ast.policies[1]
            .condition
            .as_ref()
            .expect("should have condition");
        if let crate::ast::PolicyExpr::Ref(id) = cond {
            assert_eq!(id.text, "not_flag");
        } else {
            panic!("expected Ref(not_flag), got {:?}", cond);
        }
        // Third policy: Ref("not1")
        let cond = ast.policies[2]
            .condition
            .as_ref()
            .expect("should have condition");
        if let crate::ast::PolicyExpr::Ref(id) = cond {
            assert_eq!(id.text, "not1");
        } else {
            panic!("expected Ref(not1), got {:?}", cond);
        }
    }
}
