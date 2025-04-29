use commitment_sql::CreateTableAndCommitmentMetadata;
use frame_support::assert_noop;

use crate::mock::{new_test_ext, Test};
use crate::Error;

/// Abstraction for TestParams types associated with "create table" APIs.
///
/// Can be used as a trait bound to write some common tests for all "create table" APIs.
pub trait CreateTableApiTestParams {
    /// Constructs `self` with valid parameters.
    ///
    /// Should produce an `Ok` result when the API is executed.
    fn new_valid() -> Self;

    /// Updates the sql create statement parameter to the provided value.
    fn set_sql_statement(&mut self, sql_text: String);

    /// Executes the API with these parameters.
    fn execute(self) -> Result<CreateTableAndCommitmentMetadata, Error<Test>>;
}

/// Generic test for encountering various InvalidCreateTable errors.
pub fn we_cannot_process_invalid_create_table<TestParams: CreateTableApiTestParams>() {
    new_test_ext().execute_with(|| {
        // no columns
        let mut test_params = TestParams::new_valid();
        test_params.set_sql_statement("CREATE TABLE animal.population ()".to_string());

        assert_noop!(
            test_params.execute(),
            Error::<Test>::CreateTableWithNoColumns,
        );

        // invalid identifier
        let mut test_params = TestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE global.animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT NOT NULL,
            PRIMARY KEY (animal))
            "
            .to_string(),
        );

        assert_noop!(
            test_params.execute(),
            Error::<Test>::CreateTableWithInvalidTableIdentifierCount,
        );

        // duplicate identifier
        let mut test_params = TestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            animal BIGINT NOT NULL,
            PRIMARY KEY (animal))
            "
            .to_string(),
        );

        assert_noop!(
            test_params.execute(),
            Error::<Test>::CreateTableWithDuplicateIdentifiers,
        );

        // reserved prefix
        let mut test_params = TestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            meta_population BIGINT NOT NULL,
            PRIMARY KEY (animal))
            "
            .to_string(),
        );

        assert_noop!(
            test_params.execute(),
            Error::<Test>::CreateTableWithReservedMetadataPrefix,
        );

        // nullable column
        let mut test_params = TestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT NULL,
            PRIMARY KEY (animal))
            "
            .to_string(),
        );

        assert_noop!(test_params.execute(), Error::<Test>::ColumnWithoutNotNull);

        // unsupported option
        let mut test_params = TestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT NOT NULL DEFAULT 10,
            PRIMARY KEY (animal))
            "
            .to_string(),
        );

        assert_noop!(
            test_params.execute(),
            Error::<Test>::ColumnWithUnsupportedOption,
        );
    });
}

/// Generic test for encountering various UnsupportedColumnType errors.
pub fn we_cannot_process_create_table_with_unsupported_column<
    TestParams: CreateTableApiTestParams,
>() {
    new_test_ext().execute_with(|| {
        // unsupported type parameter
        let mut test_params = TestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT NOT NULL,
            time TIMESTAMP(9) NOT NULL,
            PRIMARY KEY (animal))
            "
            .to_string(),
        );

        assert_noop!(
            test_params.execute(),
            Error::<Test>::SupportedColumnWithUnsupportedParameter,
        );

        // invalid decimal scale
        let mut test_params = TestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population DECIMAL(75, 128) NOT NULL,
            PRIMARY KEY (animal))
            "
            .to_string(),
        );

        assert_noop!(
            test_params.execute(),
            Error::<Test>::DecimalColumnWithInvalidScale,
        );

        // unsupported data type
        let mut test_params = TestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population REAL NOT NULL,
            PRIMARY KEY (animal))
            "
            .to_string(),
        );

        assert_noop!(
            test_params.execute(),
            Error::<Test>::ColumnWithUnsupportedDataType
        );
    });
}

/// Generic test for encountering the TableAlreadyExists error.
pub fn we_cannot_process_create_table_if_table_already_exists<
    TestParams: CreateTableApiTestParams,
>() {
    new_test_ext().execute_with(|| {
        let test_params = TestParams::new_valid();
        assert!(test_params.execute().is_ok());

        let test_params = TestParams::new_valid();
        assert_noop!(test_params.execute(), Error::<Test>::TableAlreadyExists,);
    })
}
