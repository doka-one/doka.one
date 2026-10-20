use std::collections::HashSet;

use commons_pg::sql_transaction::iso_to_naivedate;
use dkdto::web_types::TagType;

use crate::engine::generator::TagDefinition;
use crate::filter::filter_ast::{FilterCondition, FilterExpressionAST, FilterValue};

/// F0005 — bundle of a typed (resolved) filter AST and the tag definitions
/// it was resolved against. Carrying `definitions` makes the SQL stage's
/// precondition visible in the type system: untyped ASTs cannot reach SQL
/// generation without going through the resolver.
pub(crate) struct ResolvedFilter {
    pub ast: FilterExpressionAST,
    pub definitions: Vec<TagDefinition>,
}

/// F0005 — errors produced by the type resolver stage.
#[derive(Debug)]
pub(crate) enum FilterResolutionError {
    InvalidDateLiteral { tag: String, value: String },
    IncompatibleValueShape { tag: String, tag_type: TagType, got: &'static str },
}

/// Walk an AST and collect every attribute name referenced by any
/// `Condition` leaf — needed to know which `TagDefinition`s to load
/// before SQL emission.
pub(crate) fn collect_attribute_names(ast: &FilterExpressionAST) -> HashSet<String> {
    let mut out = HashSet::new();
    walk(ast, &mut out);
    out
}

fn walk(ast: &FilterExpressionAST, out: &mut HashSet<String>) {
    match ast {
        FilterExpressionAST::Condition(fc) => {
            out.insert(fc.attribute.clone());
        }
        FilterExpressionAST::Logical { leaves, .. } => {
            for l in leaves {
                walk(l, out);
            }
        }
    }
}

/// Rewrite every `Condition` whose attribute resolves to `TagType::Date`:
/// parse its `ValueString` via `iso_to_naivedate` and replace it with a
/// `FilterValue::ValueDate(NaiveDate)`. Other variants pass through.
pub(crate) fn resolve_filter_value_types(
    ast: FilterExpressionAST,
    definitions: Vec<TagDefinition>,
) -> Result<ResolvedFilter, FilterResolutionError> {
    let rewritten = rewrite(ast, &definitions)?;
    Ok(ResolvedFilter { ast: rewritten, definitions })
}

fn rewrite(
    ast: FilterExpressionAST,
    definitions: &[TagDefinition],
) -> Result<FilterExpressionAST, FilterResolutionError> {
    match ast {
        FilterExpressionAST::Condition(fc) => {
            let rewritten = rewrite_condition(fc, definitions)?;
            Ok(FilterExpressionAST::Condition(rewritten))
        }
        FilterExpressionAST::Logical { operator, leaves } => {
            let mut new_leaves = Vec::with_capacity(leaves.len());
            for leaf in leaves {
                let r = rewrite(*leaf, definitions)?;
                new_leaves.push(Box::new(r));
            }
            Ok(FilterExpressionAST::Logical { operator, leaves: new_leaves })
        }
    }
}

fn rewrite_condition(
    fc: FilterCondition,
    definitions: &[TagDefinition],
) -> Result<FilterCondition, FilterResolutionError> {
    let definition = definitions.iter().find(|d| d.tag_names == fc.attribute);

    let Some(def) = definition else {
        // The tag is unknown here — let the downstream "verify-tags-exist"
        // stage produce the canonical TagUnknown error. Pass through unchanged.
        return Ok(fc);
    };

    if def.tag_type != TagType::Date {
        return Ok(fc);
    }

    match &fc.value {
        FilterValue::ValueString(s) => {
            let parsed = iso_to_naivedate(s).map_err(|_| FilterResolutionError::InvalidDateLiteral {
                tag: fc.attribute.clone(),
                value: s.clone(),
            })?;
            Ok(FilterCondition { value: FilterValue::ValueDate(parsed), ..fc })
        }
        other => {
            let got = filter_value_kind(other);
            Err(FilterResolutionError::IncompatibleValueShape {
                tag: fc.attribute.clone(),
                tag_type: def.tag_type,
                got,
            })
        }
    }
}

fn filter_value_kind(v: &FilterValue) -> &'static str {
    match v {
        FilterValue::ValueInt(_) => "ValueInt",
        FilterValue::ValueString(_) => "ValueString",
        FilterValue::ValuePattern(_) => "ValuePattern",
        FilterValue::ValueBool(_) => "ValueBool",
        FilterValue::ValueDate(_) => "ValueDate",
    }
}

#[cfg(test)]
mod tests {

    // cargo test --color=always --bin document-server filter::type_resolver  [ -- --show-output]

    use std::collections::HashSet;

    use chrono::NaiveDate;
    use dkdto::web_types::TagType;
    use rs_uuid::uuid8;

    use crate::engine::generator::TagDefinition;
    use crate::filter::filter_ast::{ComparisonOperator, FilterCondition, FilterExpressionAST, FilterValue};
    use crate::filter::filter_lexer::LogicalOperator;
    use crate::filter::type_resolver::{
        FilterResolutionError, collect_attribute_names, resolve_filter_value_types,
    };

    fn cond(attribute: &str, operator: ComparisonOperator, value: FilterValue) -> Box<FilterExpressionAST> {
        Box::new(FilterExpressionAST::Condition(FilterCondition {
            key: uuid8(),
            attribute: attribute.to_string(),
            operator,
            value,
        }))
    }

    // TC-F0005-001 — `collect_attribute_names` walks AND/OR trees
    #[test]
    fn ut_f0005_001_collect_attribute_names_walks_tree() {
        let ast = FilterExpressionAST::Logical {
            operator: LogicalOperator::AND,
            leaves: vec![
                cond("birthdate", ComparisonOperator::EQ, FilterValue::ValueString("2025-12-31".to_string())),
                Box::new(FilterExpressionAST::Logical {
                    operator: LogicalOperator::OR,
                    leaves: vec![
                        cond("age", ComparisonOperator::GT, FilterValue::ValueInt(18)),
                        cond("city", ComparisonOperator::EQ, FilterValue::ValueString("Paris".to_string())),
                    ],
                }),
            ],
        };

        let got = collect_attribute_names(&ast);
        let expected: HashSet<String> =
            ["birthdate".to_string(), "age".to_string(), "city".to_string()].into_iter().collect();
        assert_eq!(expected, got);
    }

    // TC-F0005-002 — `resolve_filter_value_types` happy path
    #[test]
    fn ut_f0005_002_resolve_happy_path_rewrites_to_naivedate() {
        let ast = *cond(
            "birthdate",
            ComparisonOperator::EQ,
            FilterValue::ValueString("2025-12-31".to_string()),
        );

        let definitions =
            vec![TagDefinition { tag_names: "birthdate".to_string(), tag_type: TagType::Date }];

        let resolved = resolve_filter_value_types(ast, definitions).expect("must resolve");

        match &resolved.ast {
            FilterExpressionAST::Condition(fc) => match &fc.value {
                FilterValue::ValueDate(d) => {
                    assert_eq!(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(), *d);
                }
                other => panic!("Expected ValueDate, got {:?}", other),
            },
            other => panic!("Expected Condition, got {:?}", other),
        }
    }

    // TC-F0005-003 — Invalid date literal is rejected
    #[test]
    fn ut_f0005_003_invalid_date_literal_rejected() {
        for bad in ["2025-13-40", "not-a-date"] {
            let ast = *cond(
                "birthdate",
                ComparisonOperator::EQ,
                FilterValue::ValueString(bad.to_string()),
            );
            let definitions =
                vec![TagDefinition { tag_names: "birthdate".to_string(), tag_type: TagType::Date }];

            match resolve_filter_value_types(ast, definitions) {
                Err(FilterResolutionError::InvalidDateLiteral { tag, value }) => {
                    assert_eq!("birthdate", tag);
                    assert_eq!(bad, value);
                }
                other => panic!("Expected InvalidDateLiteral for `{}`, got {:?}", bad, other.err()),
            }
        }
    }

    // TC-F0005-004 — Wrong value shape on a Date tag
    #[test]
    fn ut_f0005_004_wrong_value_shape_on_date() {
        let ast = *cond("birthdate", ComparisonOperator::EQ, FilterValue::ValueInt(2025));
        let definitions =
            vec![TagDefinition { tag_names: "birthdate".to_string(), tag_type: TagType::Date }];

        match resolve_filter_value_types(ast, definitions) {
            Err(FilterResolutionError::IncompatibleValueShape { tag, tag_type, got }) => {
                assert_eq!("birthdate", tag);
                assert_eq!(TagType::Date, tag_type);
                assert_eq!("ValueInt", got);
            }
            other => panic!("Expected IncompatibleValueShape, got {:?}", other.err()),
        }
    }

    // TC-F0005-005 — Non-Date tags pass through unchanged
    #[test]
    fn ut_f0005_005_non_date_tags_pass_through() {
        let ast = FilterExpressionAST::Logical {
            operator: LogicalOperator::AND,
            leaves: vec![
                cond("name", ComparisonOperator::EQ, FilterValue::ValueString("alice".to_string())),
                cond("age", ComparisonOperator::GT, FilterValue::ValueInt(18)),
            ],
        };
        let definitions = vec![
            TagDefinition { tag_names: "name".to_string(), tag_type: TagType::Text },
            TagDefinition { tag_names: "age".to_string(), tag_type: TagType::Int },
        ];

        let resolved = resolve_filter_value_types(ast, definitions).expect("must resolve");

        match resolved.ast {
            FilterExpressionAST::Logical { leaves, .. } => {
                assert_eq!(2, leaves.len());
                match &*leaves[0] {
                    FilterExpressionAST::Condition(fc) => match &fc.value {
                        FilterValue::ValueString(s) => assert_eq!("alice", s),
                        other => panic!("expected ValueString, got {:?}", other),
                    },
                    other => panic!("expected Condition, got {:?}", other),
                }
                match &*leaves[1] {
                    FilterExpressionAST::Condition(fc) => match &fc.value {
                        FilterValue::ValueInt(i) => assert_eq!(18, *i),
                        other => panic!("expected ValueInt, got {:?}", other),
                    },
                    other => panic!("expected Condition, got {:?}", other),
                }
            }
            other => panic!("Expected Logical, got {:?}", other),
        }
    }

    // TC-F0005-006 — `ResolvedFilter.definitions` preserves the input
    #[test]
    fn ut_f0005_006_definitions_preserved_in_order() {
        let definitions = vec![
            TagDefinition { tag_names: "name".to_string(), tag_type: TagType::Text },
            TagDefinition { tag_names: "age".to_string(), tag_type: TagType::Int },
            TagDefinition { tag_names: "birthdate".to_string(), tag_type: TagType::Date },
        ];

        let ast = *cond("name", ComparisonOperator::EQ, FilterValue::ValueString("bob".to_string()));

        let resolved = resolve_filter_value_types(ast, definitions).expect("must resolve");

        let names: Vec<&str> = resolved.definitions.iter().map(|d| d.tag_names.as_str()).collect();
        assert_eq!(vec!["name", "age", "birthdate"], names);

        let types: Vec<TagType> = resolved.definitions.iter().map(|d| d.tag_type).collect();
        assert_eq!(vec![TagType::Text, TagType::Int, TagType::Date], types);
    }
}
