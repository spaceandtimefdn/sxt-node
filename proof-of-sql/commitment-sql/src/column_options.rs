use snafu::Snafu;
use sqlparser::ast::ColumnOption;

/// Error type for invalid column options on table definitions.
#[derive(Debug, Snafu)]
pub enum InvalidColumnOptions {
    /// Columns missing required option.
    #[snafu(display("columns missing required option: {option}"))]
    Required {
        /// The missing required option.
        option: ColumnOption,
    },
    /// Column option unsupported.
    #[snafu(display("column option unsupported: {option}"))]
    Unsupported {
        /// The unsupported column option.
        option: ColumnOption,
    },
}

/// All required column options.
const REQUIRED_OPTIONS: [ColumnOption; 1] = [ColumnOption::NotNull];

/// Returns `true` if the column option is supported.
fn column_option_is_supported(option: &ColumnOption) -> bool {
    matches!(option, ColumnOption::NotNull | ColumnOption::Comment(_))
}

/// Returns `true` if the options iterator contains the given required option.
fn options_contain_required_option<'a>(
    required_option: &ColumnOption,
    options: impl IntoIterator<Item = &'a ColumnOption>,
) -> bool {
    options.into_iter().any(|option| option == required_option)
}

/// Returns `Ok(())` if column options contain all required options and avoid unsupported options.
pub fn validate_column_options<'a>(
    options: impl IntoIterator<Item = &'a ColumnOption> + Clone,
) -> Result<(), InvalidColumnOptions> {
    if let Some(missing_required_option) = REQUIRED_OPTIONS
        .iter()
        .find(|required_option| !options_contain_required_option(required_option, options.clone()))
    {
        return Err(InvalidColumnOptions::Required {
            option: missing_required_option.clone(),
        });
    }

    if let Some(unsupported_option) = options
        .into_iter()
        .find(|&option| !column_option_is_supported(option))
    {
        return Err(InvalidColumnOptions::Unsupported {
            option: unsupported_option.clone(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use sqlparser::ast::{Expr, GeneratedAs, Value};

    use super::*;

    #[test]
    fn we_can_validate_column_options() {
        assert!(validate_column_options(&[ColumnOption::NotNull]).is_ok());

        assert!(validate_column_options(&[
            ColumnOption::Comment("Lorem ipsum".to_string()),
            ColumnOption::NotNull
        ])
        .is_ok());

        assert!(validate_column_options(&[
            ColumnOption::NotNull,
            ColumnOption::Comment("Lorem ipsum".to_string()),
        ])
        .is_ok());
    }

    #[test]
    fn we_cannot_validate_column_options_with_missing_required_option() {
        assert!(matches!(
            validate_column_options(&[]),
            Err(InvalidColumnOptions::Required { .. })
        ));
        assert!(matches!(
            validate_column_options(&[ColumnOption::Comment("Lorem ipsum".to_string())]),
            Err(InvalidColumnOptions::Required { .. })
        ));
    }

    #[test]
    fn we_cannot_validate_column_options_with_unsupported_option() {
        assert!(matches!(
            validate_column_options(&[ColumnOption::NotNull, ColumnOption::Null]),
            Err(InvalidColumnOptions::Unsupported { .. })
        ));

        assert!(matches!(
            validate_column_options(&[
                ColumnOption::Generated {
                    generated_as: GeneratedAs::ByDefault,
                    sequence_options: None,
                    generation_expr: None,
                    generation_expr_mode: None,
                    generated_keyword: false
                },
                ColumnOption::NotNull,
            ]),
            Err(InvalidColumnOptions::Unsupported { .. })
        ));

        assert!(matches!(
            validate_column_options(&[
                ColumnOption::Default(Expr::Value(Value::Null)),
                ColumnOption::NotNull,
            ]),
            Err(InvalidColumnOptions::Unsupported { .. })
        ));
    }
}
