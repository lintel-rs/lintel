//! Configuration for the dprint TypeScript / JavaScript plugin.
//!
//! See: <https://dprint.dev/plugins/typescript/config/>

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum BracePosition {
    /// Maintains the brace being on the next line or the same line.
    #[serde(rename = "maintain")]
    Maintain,
    /// Forces the brace to be on the same line.
    #[serde(rename = "sameLine")]
    SameLine,
    /// Forces the brace to be on the next line.
    #[serde(rename = "nextLine")]
    NextLine,
    /// Forces the brace to be on the next line if the same line is hanging, but otherwise uses the same line.
    #[serde(rename = "sameLineUnlessHanging")]
    SameLineUnlessHanging,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum ForceMultiLine {
    #[serde(rename = "always")]
    Always,
    #[serde(rename = "never")]
    Never,
    /// Force multiple lines only if importing more than one thing.
    #[serde(rename = "whenMultiple")]
    WhenMultiple,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum MemberSpacing {
    /// Forces a new line between members.
    #[serde(rename = "newLine")]
    NewLine,
    /// Forces a blank line between members.
    #[serde(rename = "blankLine")]
    BlankLine,
    /// Maintains whether a newline or blankline is used.
    #[serde(rename = "maintain")]
    Maintain,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum MultiLineParens {
    /// Never wrap JSX with parentheses.
    #[serde(rename = "never")]
    Never,
    /// Prefer wrapping with parentheses in most scenarios, except in function arguments and JSX attributes.
    #[serde(rename = "prefer")]
    Prefer,
    /// Always wrap JSX with parentheses if it spans multiple lines.
    #[serde(rename = "always")]
    Always,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum NewLineKind {
    /// For each file, uses the last newline kind found in the file.
    #[serde(rename = "auto")]
    Auto,
    /// Uses carriage return, line feed.
    #[serde(rename = "crlf")]
    Crlf,
    /// Uses line feed.
    #[serde(rename = "lf")]
    Lf,
    /// Uses the system standard (ex. crlf on Windows).
    #[serde(rename = "system")]
    System,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum OperatorPosition {
    /// Maintains the position of the expression.
    #[serde(rename = "maintain")]
    Maintain,
    /// Forces the whole statement to be on one line.
    #[serde(rename = "sameLine")]
    SameLine,
    /// Forces the expression to be on the next line.
    #[serde(rename = "nextLine")]
    NextLine,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum PreferHanging {
    /// Always prefers hanging regardless of the number of elements.
    #[serde(rename = "always")]
    Always,
    /// Only prefers hanging if there is a single item.
    #[serde(rename = "onlySingleItem")]
    OnlySingleItem,
    /// Never prefers hanging.
    #[serde(rename = "never")]
    Never,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum QuoteProps {
    /// Remove unnecessary quotes around property names.
    #[serde(rename = "asNeeded")]
    AsNeeded,
    /// Same as 'asNeeded', but if one property requires quotes then quote them all.
    #[serde(rename = "consistent")]
    Consistent,
    /// Preserve quotes around property names.
    #[serde(rename = "preserve")]
    Preserve,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum QuoteStyle {
    /// Always uses double quotes.
    #[serde(rename = "alwaysDouble")]
    AlwaysDouble,
    /// Always uses single quotes.
    #[serde(rename = "alwaysSingle")]
    AlwaysSingle,
    /// Prefers using double quotes except in scenarios where the string contains more double quotes than single quotes.
    #[serde(rename = "preferDouble")]
    PreferDouble,
    /// Prefers using single quotes except in scenarios where the string contains more single quotes than double quotes.
    #[serde(rename = "preferSingle")]
    PreferSingle,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum QuoteStyleKind {
    /// Prefers using double quotes except in scenarios where the string contains more double quotes than single quotes.
    #[serde(rename = "preferDouble")]
    PreferDouble,
    /// Prefers using single quotes except in scenarios where the string contains more single quotes than double quotes.
    #[serde(rename = "preferSingle")]
    PreferSingle,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum SemiColons {
    /// Always uses semi-colons where applicable.
    #[serde(rename = "always")]
    Always,
    /// Prefers semi-colons, but doesn't add one in certain scenarios such as for the last member of a single-line type literal.
    #[serde(rename = "prefer")]
    Prefer,
    /// Uses automatic semi-colon insertion. Only adds a semi-colon at the start of some expression statements when necessary. Read more: <https://standardjs.com/rules.html#semicolons>
    #[serde(rename = "asi")]
    Asi,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum SeparatorKind {
    /// Use semi-colons.
    #[serde(rename = "semiColon")]
    SemiColon,
    /// Use commas.
    #[serde(rename = "comma")]
    Comma,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum SortImportDeclarations {
    /// Maintains the current ordering.
    #[serde(rename = "maintain")]
    Maintain,
    /// Alphabetically and case sensitive.
    #[serde(rename = "caseSensitive")]
    CaseSensitive,
    /// Alphabetically and case insensitive.
    #[serde(rename = "caseInsensitive")]
    CaseInsensitive,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum SortTypeOnlyExports {
    /// Puts type-only named imports and exports first.
    #[serde(rename = "first")]
    First,
    /// Puts type-only named imports and exports last.
    #[serde(rename = "last")]
    Last,
    /// Does not sort based on if a type-only named import or export.
    #[serde(rename = "none")]
    None,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum TrailingCommas {
    /// Trailing commas should not be used.
    #[serde(rename = "never")]
    Never,
    /// Trailing commas should always be used.
    #[serde(rename = "always")]
    Always,
    /// Trailing commas should only be used in multi-line scenarios.
    #[serde(rename = "onlyMultiLine")]
    OnlyMultiLine,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum UseBraces {
    /// Uses braces if they're used. Doesn't use braces if they're not used.
    #[serde(rename = "maintain")]
    Maintain,
    /// Uses braces when the body is on a different line.
    #[serde(rename = "whenNotSingleLine")]
    WhenNotSingleLine,
    /// Forces the use of braces. Will add them if they aren't used.
    #[serde(rename = "always")]
    Always,
    /// Forces no braces when the header is one line and body is one line. Otherwise forces braces.
    #[serde(rename = "preferNone")]
    PreferNone,
}

/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum UseParentheses {
    /// Forces parentheses.
    #[serde(rename = "force")]
    Force,
    /// Maintains the current state of the parentheses.
    #[serde(rename = "maintain")]
    Maintain,
    /// Prefers not using parentheses when possible.
    #[serde(rename = "preferNone")]
    PreferNone,
}

/// Configuration for the dprint [TypeScript / JavaScript](https://dprint.dev/plugins/typescript/config/) plugin.
///
/// See: <https://dprint.dev/plugins/typescript/config/>
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[schemars(title = "TypeScript / JavaScript Plugin Configuration")]
pub struct TypeScriptConfig {
    /// Whether the configuration is not allowed to be overridden or extended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,

    /// File patterns to associate with this plugin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub associations: Option<Vec<String>>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `"never"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#argumentspreferHanging>
    #[serde(
        default,
        rename = "arguments.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub arguments_prefer_hanging: Option<PreferHanging>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#argumentspreferSingleLine>
    #[serde(
        default,
        rename = "arguments.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub arguments_prefer_single_line: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#argumentsspaceAround>
    #[serde(
        default,
        rename = "arguments.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub arguments_space_around: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#argumentstrailingCommas>
    #[serde(
        default,
        rename = "arguments.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub arguments_trailing_commas: Option<TrailingCommas>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `"never"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#arrayExpressionpreferHanging>
    #[serde(
        default,
        rename = "arrayExpression.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub array_expression_prefer_hanging: Option<PreferHanging>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#arrayExpressionpreferSingleLine>
    #[serde(
        default,
        rename = "arrayExpression.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub array_expression_prefer_single_line: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#arrayExpressionspaceAround>
    #[serde(
        default,
        rename = "arrayExpression.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub array_expression_space_around: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#arrayExpressiontrailingCommas>
    #[serde(
        default,
        rename = "arrayExpression.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub array_expression_trailing_commas: Option<TrailingCommas>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#arrayPatternpreferHanging>
    #[serde(
        default,
        rename = "arrayPattern.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub array_pattern_prefer_hanging: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#arrayPatternpreferSingleLine>
    #[serde(
        default,
        rename = "arrayPattern.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub array_pattern_prefer_single_line: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#arrayPatternspaceAround>
    #[serde(
        default,
        rename = "arrayPattern.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub array_pattern_space_around: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#arrayPatterntrailingCommas>
    #[serde(
        default,
        rename = "arrayPattern.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub array_pattern_trailing_commas: Option<TrailingCommas>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#arrowFunctionbracePosition>
    #[serde(
        default,
        rename = "arrowFunction.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub arrow_function_brace_position: Option<BracePosition>,

    /// Whether to use parentheses around a single parameter in an arrow function.
    ///
    /// Default: `"maintain"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#arrowFunctionuseParentheses>
    #[serde(
        default,
        rename = "arrowFunction.useParentheses",
        skip_serializing_if = "Option::is_none"
    )]
    pub arrow_function_use_parentheses: Option<UseParentheses>,

    /// Whether to force a line per expression when spanning multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#binaryExpressionlinePerExpression>
    #[serde(
        default,
        rename = "binaryExpression.linePerExpression",
        skip_serializing_if = "Option::is_none"
    )]
    pub binary_expression_line_per_expression: Option<bool>,

    /// Where to place the operator for expressions that span multiple lines.
    ///
    /// Default: `"nextLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#binaryExpressionoperatorPosition>
    #[serde(
        default,
        rename = "binaryExpression.operatorPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub binary_expression_operator_position: Option<OperatorPosition>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#binaryExpressionpreferSingleLine>
    #[serde(
        default,
        rename = "binaryExpression.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub binary_expression_prefer_single_line: Option<bool>,

    /// Whether to surround the operator in a binary expression with spaces.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#binaryExpressionspaceSurroundingBitwiseAndArithmeticOperator>
    #[serde(
        default,
        rename = "binaryExpression.spaceSurroundingBitwiseAndArithmeticOperator",
        skip_serializing_if = "Option::is_none"
    )]
    pub binary_expression_space_surrounding_bitwise_and_arithmetic_operator: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#bracePosition>
    #[serde(
        default,
        rename = "bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub brace_position: Option<BracePosition>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#catchClausespaceAround>
    #[serde(
        default,
        rename = "catchClause.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub catch_clause_space_around: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#classDeclarationbracePosition>
    #[serde(
        default,
        rename = "classDeclaration.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub class_declaration_brace_position: Option<BracePosition>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#classExpressionbracePosition>
    #[serde(
        default,
        rename = "classExpression.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub class_expression_brace_position: Option<BracePosition>,

    /// Forces a space after the double slash in a comment line.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#commentLineforceSpaceAfterSlashes>
    #[serde(
        default,
        rename = "commentLine.forceSpaceAfterSlashes",
        skip_serializing_if = "Option::is_none"
    )]
    pub comment_line_force_space_after_slashes: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#computedpreferSingleLine>
    #[serde(
        default,
        rename = "computed.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub computed_prefer_single_line: Option<bool>,

    /// Where to place the operator for expressions that span multiple lines.
    ///
    /// Default: `"nextLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#conditionalExpressionoperatorPosition>
    #[serde(
        default,
        rename = "conditionalExpression.operatorPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub conditional_expression_operator_position: Option<OperatorPosition>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#conditionalExpressionpreferSingleLine>
    #[serde(
        default,
        rename = "conditionalExpression.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub conditional_expression_prefer_single_line: Option<bool>,

    /// Where to place the operator for expressions that span multiple lines.
    ///
    /// Default: `"nextLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#conditionalTypeoperatorPosition>
    #[serde(
        default,
        rename = "conditionalType.operatorPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub conditional_type_operator_position: Option<OperatorPosition>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#conditionalTypepreferSingleLine>
    #[serde(
        default,
        rename = "conditionalType.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub conditional_type_prefer_single_line: Option<bool>,

    /// Whether to add a space after the `new` keyword in a construct signature.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#constructSignaturespaceAfterNewKeyword>
    #[serde(
        default,
        rename = "constructSignature.spaceAfterNewKeyword",
        skip_serializing_if = "Option::is_none"
    )]
    pub construct_signature_space_after_new_keyword: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#constructorbracePosition>
    #[serde(
        default,
        rename = "constructor.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub constructor_brace_position: Option<BracePosition>,

    /// Whether to add a space before the parentheses of a constructor.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#constructorspaceBeforeParentheses>
    #[serde(
        default,
        rename = "constructor.spaceBeforeParentheses",
        skip_serializing_if = "Option::is_none"
    )]
    pub constructor_space_before_parentheses: Option<bool>,

    /// Whether to add a space after the `new` keyword in a constructor type.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#constructorTypespaceAfterNewKeyword>
    #[serde(
        default,
        rename = "constructorType.spaceAfterNewKeyword",
        skip_serializing_if = "Option::is_none"
    )]
    pub constructor_type_space_after_new_keyword: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#decoratorspreferSingleLine>
    #[serde(
        default,
        rename = "decorators.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub decorators_prefer_single_line: Option<bool>,

    /// Top level configuration that sets the configuration to what is used in Deno.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#deno>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deno: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#doWhileStatementbracePosition>
    #[serde(
        default,
        rename = "doWhileStatement.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub do_while_statement_brace_position: Option<BracePosition>,

    /// Where to place the next control flow within a control flow statement.
    ///
    /// Default: `"sameLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#doWhileStatementnextControlFlowPosition>
    #[serde(
        default,
        rename = "doWhileStatement.nextControlFlowPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub do_while_statement_next_control_flow_position: Option<OperatorPosition>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#doWhileStatementpreferHanging>
    #[serde(
        default,
        rename = "doWhileStatement.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub do_while_statement_prefer_hanging: Option<bool>,

    /// Whether to add a space after the `while` keyword in a do while statement.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#doWhileStatementspaceAfterWhileKeyword>
    #[serde(
        default,
        rename = "doWhileStatement.spaceAfterWhileKeyword",
        skip_serializing_if = "Option::is_none"
    )]
    pub do_while_statement_space_after_while_keyword: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#doWhileStatementspaceAround>
    #[serde(
        default,
        rename = "doWhileStatement.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub do_while_statement_space_around: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#enumDeclarationbracePosition>
    #[serde(
        default,
        rename = "enumDeclaration.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub enum_declaration_brace_position: Option<BracePosition>,

    /// How to space the members of an enum.
    ///
    /// Default: `"maintain"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#enumDeclarationmemberSpacing>
    #[serde(
        default,
        rename = "enumDeclaration.memberSpacing",
        skip_serializing_if = "Option::is_none"
    )]
    pub enum_declaration_member_spacing: Option<MemberSpacing>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#enumDeclarationtrailingCommas>
    #[serde(
        default,
        rename = "enumDeclaration.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub enum_declaration_trailing_commas: Option<TrailingCommas>,

    /// If code import/export specifiers should be forced to be on multiple lines.
    ///
    /// Default: `"never"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#exportDeclarationforceMultiLine>
    #[serde(
        default,
        rename = "exportDeclaration.forceMultiLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub export_declaration_force_multi_line: Option<ForceMultiLine>,

    /// If code should be forced to be on a single line if able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#exportDeclarationforceSingleLine>
    #[serde(
        default,
        rename = "exportDeclaration.forceSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub export_declaration_force_single_line: Option<bool>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#exportDeclarationpreferHanging>
    #[serde(
        default,
        rename = "exportDeclaration.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub export_declaration_prefer_hanging: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#exportDeclarationpreferSingleLine>
    #[serde(
        default,
        rename = "exportDeclaration.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub export_declaration_prefer_single_line: Option<bool>,

    /// The kind of sort ordering to use.
    ///
    /// Default: `"caseInsensitive"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#exportDeclarationsortNamedExports>
    #[serde(
        default,
        rename = "exportDeclaration.sortNamedExports",
        skip_serializing_if = "Option::is_none"
    )]
    pub export_declaration_sort_named_exports: Option<SortImportDeclarations>,

    /// The kind of sort ordering to use for typed imports and exports.
    ///
    /// Default: `"none"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#exportDeclarationsortTypeOnlyExports>
    #[serde(
        default,
        rename = "exportDeclaration.sortTypeOnlyExports",
        skip_serializing_if = "Option::is_none"
    )]
    pub export_declaration_sort_type_only_exports: Option<SortTypeOnlyExports>,

    /// Whether to add spaces around named exports in an export declaration.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#exportDeclarationspaceSurroundingNamedExports>
    #[serde(
        default,
        rename = "exportDeclaration.spaceSurroundingNamedExports",
        skip_serializing_if = "Option::is_none"
    )]
    pub export_declaration_space_surrounding_named_exports: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#exportDeclarationtrailingCommas>
    #[serde(
        default,
        rename = "exportDeclaration.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub export_declaration_trailing_commas: Option<TrailingCommas>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#extendsClausepreferHanging>
    #[serde(
        default,
        rename = "extendsClause.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub extends_clause_prefer_hanging: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forInStatementbracePosition>
    #[serde(
        default,
        rename = "forInStatement.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_in_statement_brace_position: Option<BracePosition>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forInStatementpreferHanging>
    #[serde(
        default,
        rename = "forInStatement.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_in_statement_prefer_hanging: Option<bool>,

    /// Where to place the expression of a statement that could possibly be on one line (ex. `if (true) console.log(5);`).
    ///
    /// Default: `"maintain"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forInStatementsingleBodyPosition>
    #[serde(
        default,
        rename = "forInStatement.singleBodyPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_in_statement_single_body_position: Option<OperatorPosition>,

    /// Whether to add a space after the `for` keyword in a "for in" statement.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forInStatementspaceAfterForKeyword>
    #[serde(
        default,
        rename = "forInStatement.spaceAfterForKeyword",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_in_statement_space_after_for_keyword: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forInStatementspaceAround>
    #[serde(
        default,
        rename = "forInStatement.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_in_statement_space_around: Option<bool>,

    /// If braces should be used or not.
    ///
    /// Default: `"whenNotSingleLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forInStatementuseBraces>
    #[serde(
        default,
        rename = "forInStatement.useBraces",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_in_statement_use_braces: Option<UseBraces>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forOfStatementbracePosition>
    #[serde(
        default,
        rename = "forOfStatement.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_of_statement_brace_position: Option<BracePosition>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forOfStatementpreferHanging>
    #[serde(
        default,
        rename = "forOfStatement.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_of_statement_prefer_hanging: Option<bool>,

    /// Where to place the expression of a statement that could possibly be on one line (ex. `if (true) console.log(5);`).
    ///
    /// Default: `"maintain"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forOfStatementsingleBodyPosition>
    #[serde(
        default,
        rename = "forOfStatement.singleBodyPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_of_statement_single_body_position: Option<OperatorPosition>,

    /// Whether to add a space after the `for` keyword in a "for of" statement.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forOfStatementspaceAfterForKeyword>
    #[serde(
        default,
        rename = "forOfStatement.spaceAfterForKeyword",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_of_statement_space_after_for_keyword: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forOfStatementspaceAround>
    #[serde(
        default,
        rename = "forOfStatement.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_of_statement_space_around: Option<bool>,

    /// If braces should be used or not.
    ///
    /// Default: `"whenNotSingleLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forOfStatementuseBraces>
    #[serde(
        default,
        rename = "forOfStatement.useBraces",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_of_statement_use_braces: Option<UseBraces>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forStatementbracePosition>
    #[serde(
        default,
        rename = "forStatement.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_statement_brace_position: Option<BracePosition>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forStatementpreferHanging>
    #[serde(
        default,
        rename = "forStatement.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_statement_prefer_hanging: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forStatementpreferSingleLine>
    #[serde(
        default,
        rename = "forStatement.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_statement_prefer_single_line: Option<bool>,

    /// Where to place the expression of a statement that could possibly be on one line (ex. `if (true) console.log(5);`).
    ///
    /// Default: `"maintain"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forStatementsingleBodyPosition>
    #[serde(
        default,
        rename = "forStatement.singleBodyPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_statement_single_body_position: Option<OperatorPosition>,

    /// Whether to add a space after the `for` keyword in a "for" statement.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forStatementspaceAfterForKeyword>
    #[serde(
        default,
        rename = "forStatement.spaceAfterForKeyword",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_statement_space_after_for_keyword: Option<bool>,

    /// Whether to add a space after the semi-colons in a "for" statement.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forStatementspaceAfterSemiColons>
    #[serde(
        default,
        rename = "forStatement.spaceAfterSemiColons",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_statement_space_after_semi_colons: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forStatementspaceAround>
    #[serde(
        default,
        rename = "forStatement.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_statement_space_around: Option<bool>,

    /// If braces should be used or not.
    ///
    /// Default: `"whenNotSingleLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#forStatementuseBraces>
    #[serde(
        default,
        rename = "forStatement.useBraces",
        skip_serializing_if = "Option::is_none"
    )]
    pub for_statement_use_braces: Option<UseBraces>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#functionDeclarationbracePosition>
    #[serde(
        default,
        rename = "functionDeclaration.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub function_declaration_brace_position: Option<BracePosition>,

    /// Whether to add a space before the parentheses of a function declaration.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#functionDeclarationspaceBeforeParentheses>
    #[serde(
        default,
        rename = "functionDeclaration.spaceBeforeParentheses",
        skip_serializing_if = "Option::is_none"
    )]
    pub function_declaration_space_before_parentheses: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#functionExpressionbracePosition>
    #[serde(
        default,
        rename = "functionExpression.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub function_expression_brace_position: Option<BracePosition>,

    /// Whether to add a space after the function keyword of a function expression.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#functionExpressionspaceAfterFunctionKeyword>
    #[serde(
        default,
        rename = "functionExpression.spaceAfterFunctionKeyword",
        skip_serializing_if = "Option::is_none"
    )]
    pub function_expression_space_after_function_keyword: Option<bool>,

    /// Whether to add a space before the parentheses of a function expression.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#functionExpressionspaceBeforeParentheses>
    #[serde(
        default,
        rename = "functionExpression.spaceBeforeParentheses",
        skip_serializing_if = "Option::is_none"
    )]
    pub function_expression_space_before_parentheses: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#getAccessorbracePosition>
    #[serde(
        default,
        rename = "getAccessor.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub get_accessor_brace_position: Option<BracePosition>,

    /// Whether to add a space before the parentheses of a get accessor.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#getAccessorspaceBeforeParentheses>
    #[serde(
        default,
        rename = "getAccessor.spaceBeforeParentheses",
        skip_serializing_if = "Option::is_none"
    )]
    pub get_accessor_space_before_parentheses: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#ifStatementbracePosition>
    #[serde(
        default,
        rename = "ifStatement.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub if_statement_brace_position: Option<BracePosition>,

    /// Where to place the next control flow within a control flow statement.
    ///
    /// Default: `"sameLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#ifStatementnextControlFlowPosition>
    #[serde(
        default,
        rename = "ifStatement.nextControlFlowPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub if_statement_next_control_flow_position: Option<OperatorPosition>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#ifStatementpreferHanging>
    #[serde(
        default,
        rename = "ifStatement.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub if_statement_prefer_hanging: Option<bool>,

    /// Where to place the expression of a statement that could possibly be on one line (ex. `if (true) console.log(5);`).
    ///
    /// Default: `"maintain"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#ifStatementsingleBodyPosition>
    #[serde(
        default,
        rename = "ifStatement.singleBodyPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub if_statement_single_body_position: Option<OperatorPosition>,

    /// Whether to add a space after the `if` keyword in an "if" statement.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#ifStatementspaceAfterIfKeyword>
    #[serde(
        default,
        rename = "ifStatement.spaceAfterIfKeyword",
        skip_serializing_if = "Option::is_none"
    )]
    pub if_statement_space_after_if_keyword: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#ifStatementspaceAround>
    #[serde(
        default,
        rename = "ifStatement.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub if_statement_space_around: Option<bool>,

    /// If braces should be used or not.
    ///
    /// Default: `"whenNotSingleLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#ifStatementuseBraces>
    #[serde(
        default,
        rename = "ifStatement.useBraces",
        skip_serializing_if = "Option::is_none"
    )]
    pub if_statement_use_braces: Option<UseBraces>,

    /// The text to use for a file ignore comment (ex. `// dprint-ignore-file`).
    ///
    /// Default: `"dprint-ignore-file"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#ignoreFileCommentText>
    #[serde(
        default,
        rename = "ignoreFileCommentText",
        skip_serializing_if = "Option::is_none"
    )]
    pub ignore_file_comment_text: Option<String>,

    /// The text to use for an ignore comment (ex. `// dprint-ignore`).
    ///
    /// Default: `"dprint-ignore"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#ignoreNodeCommentText>
    #[serde(
        default,
        rename = "ignoreNodeCommentText",
        skip_serializing_if = "Option::is_none"
    )]
    pub ignore_node_comment_text: Option<String>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#implementsClausepreferHanging>
    #[serde(
        default,
        rename = "implementsClause.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub implements_clause_prefer_hanging: Option<bool>,

    /// If code import/export specifiers should be forced to be on multiple lines.
    ///
    /// Default: `"never"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#importDeclarationforceMultiLine>
    #[serde(
        default,
        rename = "importDeclaration.forceMultiLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub import_declaration_force_multi_line: Option<ForceMultiLine>,

    /// If code should be forced to be on a single line if able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#importDeclarationforceSingleLine>
    #[serde(
        default,
        rename = "importDeclaration.forceSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub import_declaration_force_single_line: Option<bool>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#importDeclarationpreferHanging>
    #[serde(
        default,
        rename = "importDeclaration.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub import_declaration_prefer_hanging: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#importDeclarationpreferSingleLine>
    #[serde(
        default,
        rename = "importDeclaration.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub import_declaration_prefer_single_line: Option<bool>,

    /// The kind of sort ordering to use.
    ///
    /// Default: `"caseInsensitive"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#importDeclarationsortNamedImports>
    #[serde(
        default,
        rename = "importDeclaration.sortNamedImports",
        skip_serializing_if = "Option::is_none"
    )]
    pub import_declaration_sort_named_imports: Option<SortImportDeclarations>,

    /// The kind of sort ordering to use for typed imports and exports.
    ///
    /// Default: `"none"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#importDeclarationsortTypeOnlyImports>
    #[serde(
        default,
        rename = "importDeclaration.sortTypeOnlyImports",
        skip_serializing_if = "Option::is_none"
    )]
    pub import_declaration_sort_type_only_imports: Option<SortTypeOnlyExports>,

    /// Whether to add spaces around named imports in an import declaration.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#importDeclarationspaceSurroundingNamedImports>
    #[serde(
        default,
        rename = "importDeclaration.spaceSurroundingNamedImports",
        skip_serializing_if = "Option::is_none"
    )]
    pub import_declaration_space_surrounding_named_imports: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#importDeclarationtrailingCommas>
    #[serde(
        default,
        rename = "importDeclaration.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub import_declaration_trailing_commas: Option<TrailingCommas>,

    /// The number of columns for an indent.
    ///
    /// Default: `2`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#indentWidth>
    #[serde(
        default,
        rename = "indentWidth",
        skip_serializing_if = "Option::is_none"
    )]
    pub indent_width: Option<u32>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#interfaceDeclarationbracePosition>
    #[serde(
        default,
        rename = "interfaceDeclaration.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub interface_declaration_brace_position: Option<BracePosition>,

    /// If the end angle bracket of a jsx open element or self closing element should be on the same or next line when the attributes span multiple lines.
    ///
    /// Default: `"nextLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#jsxbracketPosition>
    #[serde(
        default,
        rename = "jsx.bracketPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub jsx_bracket_position: Option<OperatorPosition>,

    /// Forces newlines surrounding the content of JSX elements.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#jsxforceNewLinesSurroundingContent>
    #[serde(
        default,
        rename = "jsx.forceNewLinesSurroundingContent",
        skip_serializing_if = "Option::is_none"
    )]
    pub jsx_force_new_lines_surrounding_content: Option<bool>,

    /// Surrounds the top-most JSX element or fragment in parentheses when it spans multiple lines.
    ///
    /// Default: `"prefer"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#jsxmultiLineParens>
    #[serde(
        default,
        rename = "jsx.multiLineParens",
        skip_serializing_if = "Option::is_none"
    )]
    pub jsx_multi_line_parens: Option<MultiLineParens>,

    /// How to use single or double quotes in JSX attributes.
    ///
    /// Default: `"preferDouble"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#jsxquoteStyle>
    #[serde(
        default,
        rename = "jsx.quoteStyle",
        skip_serializing_if = "Option::is_none"
    )]
    pub jsx_quote_style: Option<QuoteStyleKind>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#jsxAttributespreferHanging>
    #[serde(
        default,
        rename = "jsxAttributes.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub jsx_attributes_prefer_hanging: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#jsxAttributespreferSingleLine>
    #[serde(
        default,
        rename = "jsxAttributes.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub jsx_attributes_prefer_single_line: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#jsxElementpreferSingleLine>
    #[serde(
        default,
        rename = "jsxElement.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub jsx_element_prefer_single_line: Option<bool>,

    /// Whether to add a space surrounding the expression of a JSX container.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#jsxExpressionContainerspaceSurroundingExpression>
    #[serde(
        default,
        rename = "jsxExpressionContainer.spaceSurroundingExpression",
        skip_serializing_if = "Option::is_none"
    )]
    pub jsx_expression_container_space_surrounding_expression: Option<bool>,

    /// If the end angle bracket of a jsx open element or self closing element should be on the same or next line when the attributes span multiple lines.
    ///
    /// Default: `"nextLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#jsxOpeningElementbracketPosition>
    #[serde(
        default,
        rename = "jsxOpeningElement.bracketPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub jsx_opening_element_bracket_position: Option<OperatorPosition>,

    /// If the end angle bracket of a jsx open element or self closing element should be on the same or next line when the attributes span multiple lines.
    ///
    /// Default: `"nextLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#jsxSelfClosingElementbracketPosition>
    #[serde(
        default,
        rename = "jsxSelfClosingElement.bracketPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub jsx_self_closing_element_bracket_position: Option<OperatorPosition>,

    /// Whether to add a space before a JSX element's slash when self closing.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#jsxSelfClosingElementspaceBeforeSlash>
    #[serde(
        default,
        rename = "jsxSelfClosingElement.spaceBeforeSlash",
        skip_serializing_if = "Option::is_none"
    )]
    pub jsx_self_closing_element_space_before_slash: Option<bool>,

    /// The width of a line the printer will try to stay under. Note that the printer may exceed this width in certain cases.
    ///
    /// Default: `120`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#lineWidth>
    #[serde(default, rename = "lineWidth", skip_serializing_if = "Option::is_none")]
    pub line_width: Option<u32>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#mappedTypepreferSingleLine>
    #[serde(
        default,
        rename = "mappedType.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub mapped_type_prefer_single_line: Option<bool>,

    /// Whether to force a line per expression when spanning multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#memberExpressionlinePerExpression>
    #[serde(
        default,
        rename = "memberExpression.linePerExpression",
        skip_serializing_if = "Option::is_none"
    )]
    pub member_expression_line_per_expression: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#memberExpressionpreferSingleLine>
    #[serde(
        default,
        rename = "memberExpression.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub member_expression_prefer_single_line: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#methodbracePosition>
    #[serde(
        default,
        rename = "method.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub method_brace_position: Option<BracePosition>,

    /// Whether to add a space before the parentheses of a method.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#methodspaceBeforeParentheses>
    #[serde(
        default,
        rename = "method.spaceBeforeParentheses",
        skip_serializing_if = "Option::is_none"
    )]
    pub method_space_before_parentheses: Option<bool>,

    /// The kind of sort ordering to use.
    ///
    /// Default: `"caseInsensitive"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#modulesortExportDeclarations>
    #[serde(
        default,
        rename = "module.sortExportDeclarations",
        skip_serializing_if = "Option::is_none"
    )]
    pub module_sort_export_declarations: Option<SortImportDeclarations>,

    /// The kind of sort ordering to use.
    ///
    /// Default: `"caseInsensitive"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#modulesortImportDeclarations>
    #[serde(
        default,
        rename = "module.sortImportDeclarations",
        skip_serializing_if = "Option::is_none"
    )]
    pub module_sort_import_declarations: Option<SortImportDeclarations>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#moduleDeclarationbracePosition>
    #[serde(
        default,
        rename = "moduleDeclaration.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub module_declaration_brace_position: Option<BracePosition>,

    /// The kind of newline to use.
    ///
    /// Default: `"lf"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#newLineKind>
    #[serde(
        default,
        rename = "newLineKind",
        skip_serializing_if = "Option::is_none"
    )]
    pub new_line_kind: Option<NewLineKind>,

    /// Where to place the next control flow within a control flow statement.
    ///
    /// Default: `"sameLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#nextControlFlowPosition>
    #[serde(
        default,
        rename = "nextControlFlowPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub next_control_flow_position: Option<OperatorPosition>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#objectExpressionpreferHanging>
    #[serde(
        default,
        rename = "objectExpression.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub object_expression_prefer_hanging: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#objectExpressionpreferSingleLine>
    #[serde(
        default,
        rename = "objectExpression.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub object_expression_prefer_single_line: Option<bool>,

    /// Whether to add a space surrounding the properties of a single line object expression.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#objectExpressionspaceSurroundingProperties>
    #[serde(
        default,
        rename = "objectExpression.spaceSurroundingProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub object_expression_space_surrounding_properties: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#objectExpressiontrailingCommas>
    #[serde(
        default,
        rename = "objectExpression.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub object_expression_trailing_commas: Option<TrailingCommas>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#objectPatternpreferHanging>
    #[serde(
        default,
        rename = "objectPattern.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub object_pattern_prefer_hanging: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#objectPatternpreferSingleLine>
    #[serde(
        default,
        rename = "objectPattern.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub object_pattern_prefer_single_line: Option<bool>,

    /// Whether to add a space surrounding the properties of a single line object pattern.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#objectPatternspaceSurroundingProperties>
    #[serde(
        default,
        rename = "objectPattern.spaceSurroundingProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub object_pattern_space_surrounding_properties: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#objectPatterntrailingCommas>
    #[serde(
        default,
        rename = "objectPattern.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub object_pattern_trailing_commas: Option<TrailingCommas>,

    /// Where to place the operator for expressions that span multiple lines.
    ///
    /// Default: `"nextLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#operatorPosition>
    #[serde(
        default,
        rename = "operatorPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub operator_position: Option<OperatorPosition>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `"never"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#parameterspreferHanging>
    #[serde(
        default,
        rename = "parameters.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub parameters_prefer_hanging: Option<PreferHanging>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#parameterspreferSingleLine>
    #[serde(
        default,
        rename = "parameters.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub parameters_prefer_single_line: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#parametersspaceAround>
    #[serde(
        default,
        rename = "parameters.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub parameters_space_around: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#parameterstrailingCommas>
    #[serde(
        default,
        rename = "parameters.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub parameters_trailing_commas: Option<TrailingCommas>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#parenExpressionspaceAround>
    #[serde(
        default,
        rename = "parenExpression.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub paren_expression_space_around: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#parenthesespreferSingleLine>
    #[serde(
        default,
        rename = "parentheses.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub parentheses_prefer_single_line: Option<bool>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#preferHanging>
    #[serde(
        default,
        rename = "preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub prefer_hanging: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#preferSingleLine>
    #[serde(
        default,
        rename = "preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub prefer_single_line: Option<bool>,

    /// Change when properties in objects are quoted.
    ///
    /// Default: `"preserve"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#quoteProps>
    #[serde(
        default,
        rename = "quoteProps",
        skip_serializing_if = "Option::is_none"
    )]
    pub quote_props: Option<QuoteProps>,

    /// How to use single or double quotes.
    ///
    /// Default: `"alwaysDouble"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#quoteStyle>
    #[serde(
        default,
        rename = "quoteStyle",
        skip_serializing_if = "Option::is_none"
    )]
    pub quote_style: Option<QuoteStyle>,

    /// How semi-colons should be used.
    ///
    /// Default: `"prefer"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#semiColons>
    #[serde(
        default,
        rename = "semiColons",
        skip_serializing_if = "Option::is_none"
    )]
    pub semi_colons: Option<SemiColons>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#sequenceExpressionpreferHanging>
    #[serde(
        default,
        rename = "sequenceExpression.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub sequence_expression_prefer_hanging: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#setAccessorbracePosition>
    #[serde(
        default,
        rename = "setAccessor.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub set_accessor_brace_position: Option<BracePosition>,

    /// Whether to add a space before the parentheses of a set accessor.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#setAccessorspaceBeforeParentheses>
    #[serde(
        default,
        rename = "setAccessor.spaceBeforeParentheses",
        skip_serializing_if = "Option::is_none"
    )]
    pub set_accessor_space_before_parentheses: Option<bool>,

    /// Where to place the expression of a statement that could possibly be on one line (ex. `if (true) console.log(5);`).
    ///
    /// Default: `"maintain"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#singleBodyPosition>
    #[serde(
        default,
        rename = "singleBodyPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub single_body_position: Option<OperatorPosition>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#spaceAround>
    #[serde(
        default,
        rename = "spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub space_around: Option<bool>,

    /// Whether to add a space surrounding the properties of single line object-like nodes.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#spaceSurroundingProperties>
    #[serde(
        default,
        rename = "spaceSurroundingProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub space_surrounding_properties: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#staticBlockbracePosition>
    #[serde(
        default,
        rename = "staticBlock.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub static_block_brace_position: Option<BracePosition>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#switchCasebracePosition>
    #[serde(
        default,
        rename = "switchCase.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub switch_case_brace_position: Option<BracePosition>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#switchStatementbracePosition>
    #[serde(
        default,
        rename = "switchStatement.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub switch_statement_brace_position: Option<BracePosition>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#switchStatementpreferHanging>
    #[serde(
        default,
        rename = "switchStatement.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub switch_statement_prefer_hanging: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#switchStatementspaceAround>
    #[serde(
        default,
        rename = "switchStatement.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub switch_statement_space_around: Option<bool>,

    /// Whether to add a space before the literal in a tagged template.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#taggedTemplatespaceBeforeLiteral>
    #[serde(
        default,
        rename = "taggedTemplate.spaceBeforeLiteral",
        skip_serializing_if = "Option::is_none"
    )]
    pub tagged_template_space_before_literal: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#trailingCommas>
    #[serde(
        default,
        rename = "trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub trailing_commas: Option<TrailingCommas>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#tryStatementbracePosition>
    #[serde(
        default,
        rename = "tryStatement.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub try_statement_brace_position: Option<BracePosition>,

    /// Where to place the next control flow within a control flow statement.
    ///
    /// Default: `"sameLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#tryStatementnextControlFlowPosition>
    #[serde(
        default,
        rename = "tryStatement.nextControlFlowPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub try_statement_next_control_flow_position: Option<OperatorPosition>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `"never"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#tupleTypepreferHanging>
    #[serde(
        default,
        rename = "tupleType.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub tuple_type_prefer_hanging: Option<PreferHanging>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#tupleTypepreferSingleLine>
    #[serde(
        default,
        rename = "tupleType.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub tuple_type_prefer_single_line: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#tupleTypespaceAround>
    #[serde(
        default,
        rename = "tupleType.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub tuple_type_space_around: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#tupleTypetrailingCommas>
    #[serde(
        default,
        rename = "tupleType.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub tuple_type_trailing_commas: Option<TrailingCommas>,

    /// Whether to add a space before the colon of a type annotation.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeAnnotationspaceBeforeColon>
    #[serde(
        default,
        rename = "typeAnnotation.spaceBeforeColon",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_annotation_space_before_colon: Option<bool>,

    /// Whether to add a space before the expression in a type assertion.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeAssertionspaceBeforeExpression>
    #[serde(
        default,
        rename = "typeAssertion.spaceBeforeExpression",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_assertion_space_before_expression: Option<bool>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeLiteralpreferHanging>
    #[serde(
        default,
        rename = "typeLiteral.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_literal_prefer_hanging: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeLiteralpreferSingleLine>
    #[serde(
        default,
        rename = "typeLiteral.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_literal_prefer_single_line: Option<bool>,

    /// The kind of separator to use in type literals.
    ///
    /// Default: `"semiColon"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeLiteralseparatorKind>
    #[serde(
        default,
        rename = "typeLiteral.separatorKind",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_literal_separator_kind: Option<SeparatorKind>,

    /// The kind of separator to use in type literals.
    ///
    /// Default: `"semiColon"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeLiteralseparatorKindmultiLine>
    #[serde(
        default,
        rename = "typeLiteral.separatorKind.multiLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_literal_separator_kind_multi_line: Option<SeparatorKind>,

    /// The kind of separator to use in type literals.
    ///
    /// Default: `"semiColon"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeLiteralseparatorKindsingleLine>
    #[serde(
        default,
        rename = "typeLiteral.separatorKind.singleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_literal_separator_kind_single_line: Option<SeparatorKind>,

    /// Whether to add a space surrounding the properties of a single line type literal.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeLiteralspaceSurroundingProperties>
    #[serde(
        default,
        rename = "typeLiteral.spaceSurroundingProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_literal_space_surrounding_properties: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeLiteraltrailingCommas>
    #[serde(
        default,
        rename = "typeLiteral.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_literal_trailing_commas: Option<TrailingCommas>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `"never"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeParameterspreferHanging>
    #[serde(
        default,
        rename = "typeParameters.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_parameters_prefer_hanging: Option<PreferHanging>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeParameterspreferSingleLine>
    #[serde(
        default,
        rename = "typeParameters.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_parameters_prefer_single_line: Option<bool>,

    /// If trailing commas should be used.
    ///
    /// Default: `"onlyMultiLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#typeParameterstrailingCommas>
    #[serde(
        default,
        rename = "typeParameters.trailingCommas",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_parameters_trailing_commas: Option<TrailingCommas>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#unionAndIntersectionTypepreferHanging>
    #[serde(
        default,
        rename = "unionAndIntersectionType.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub union_and_intersection_type_prefer_hanging: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#unionAndIntersectionTypepreferSingleLine>
    #[serde(
        default,
        rename = "unionAndIntersectionType.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub union_and_intersection_type_prefer_single_line: Option<bool>,

    /// If braces should be used or not.
    ///
    /// Default: `"whenNotSingleLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#useBraces>
    #[serde(default, rename = "useBraces", skip_serializing_if = "Option::is_none")]
    pub use_braces: Option<UseBraces>,

    /// Whether to use tabs (true) or spaces (false).
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#useTabs>
    #[serde(default, rename = "useTabs", skip_serializing_if = "Option::is_none")]
    pub use_tabs: Option<bool>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#variableStatementpreferHanging>
    #[serde(
        default,
        rename = "variableStatement.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub variable_statement_prefer_hanging: Option<bool>,

    /// If code should revert back from being on multiple lines to being on a single line when able.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#variableStatementpreferSingleLine>
    #[serde(
        default,
        rename = "variableStatement.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub variable_statement_prefer_single_line: Option<bool>,

    /// Where to place the opening brace.
    ///
    /// Default: `"sameLineUnlessHanging"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#whileStatementbracePosition>
    #[serde(
        default,
        rename = "whileStatement.bracePosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub while_statement_brace_position: Option<BracePosition>,

    /// Set to prefer hanging indentation when exceeding the line width instead of making code split up on multiple lines.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#whileStatementpreferHanging>
    #[serde(
        default,
        rename = "whileStatement.preferHanging",
        skip_serializing_if = "Option::is_none"
    )]
    pub while_statement_prefer_hanging: Option<bool>,

    /// Where to place the expression of a statement that could possibly be on one line (ex. `if (true) console.log(5);`).
    ///
    /// Default: `"maintain"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#whileStatementsingleBodyPosition>
    #[serde(
        default,
        rename = "whileStatement.singleBodyPosition",
        skip_serializing_if = "Option::is_none"
    )]
    pub while_statement_single_body_position: Option<OperatorPosition>,

    /// Whether to add a space after the `while` keyword in a while statement.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#whileStatementspaceAfterWhileKeyword>
    #[serde(
        default,
        rename = "whileStatement.spaceAfterWhileKeyword",
        skip_serializing_if = "Option::is_none"
    )]
    pub while_statement_space_after_while_keyword: Option<bool>,

    /// Whether to place spaces around enclosed expressions.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#whileStatementspaceAround>
    #[serde(
        default,
        rename = "whileStatement.spaceAround",
        skip_serializing_if = "Option::is_none"
    )]
    pub while_statement_space_around: Option<bool>,

    /// If braces should be used or not.
    ///
    /// Default: `"whenNotSingleLine"`
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/#whileStatementuseBraces>
    #[serde(
        default,
        rename = "whileStatement.useBraces",
        skip_serializing_if = "Option::is_none"
    )]
    pub while_statement_use_braces: Option<UseBraces>,

    /// Additional plugin-specific settings not covered by the typed fields.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
