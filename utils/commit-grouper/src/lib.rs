//! TODO make this better
//!

use std::path::PathBuf;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::sp_runtime::Serialize;
use frame_support::BoundedVec;
use proof_of_sql_commitment_map::{
    TableCommitmentBytes,
    TableCommitmentBytesPerCommitmentScheme,
    TableCommitmentMaxLength,
};
use scale_info::TypeInfo;
use serde::Deserialize;
use snafu::{OptionExt, ResultExt, Snafu};
use sxt_core::tables::{TableIdentifier, TableName, TableNamespace};

/// TODO
#[derive(Debug, Snafu)]
pub enum CommitmentParserError {
    /// Failed to serialize `PublicParameters`.
    #[snafu(display("No file name from path buffer"))]
    NoFileNameError,

    /// TODO
    #[snafu(display("The file name was formatted incorrectly, expecting [PREFIX]-[NAMESPACE]-[TABLE NAME]-[COMMITMENT SCHEME]-commitment.txt"))]
    FileNameFormatError,

    /// TODO
    #[snafu(display("Error reading commitment file"))]
    ErrorReadingCommitmentFile,

    /// TODO
    #[snafu(display("Error constructing a bounded vector for the data"))]
    BoundedVecError,

    /// TODO
    #[snafu(display("Error creating table namespace"))]
    TableNamespaceError,

    /// TODO
    #[snafu(display("Error creating table namespace"))]
    TableNameError,

    /// TODO
    #[snafu(display("InvalidCommitmentScheme"))]
    InvalidCommitmentScheme,

    /// TODO
    #[snafu(display("Glob Error"))]
    GlobError,

    /// TODO
    #[snafu(display("No commits found"))]
    NoCommitsFoundError,
}

/// todo
pub struct CommitmentParser {}

impl CommitmentParser {
    /// Read a single commit from a path buffer, expecting paths in the following format: [PREFIX]-[NAMESPACE]-[TABLE NAME]-[COMMITMENT SCHEME]-commitment.txt
    pub fn single_commit_from_path_buf(p: PathBuf) -> Result<SingleCommit, CommitmentParserError> {
        let file_name: String = p
            .clone()
            .file_name()
            .context(NoFileNameSnafu)?
            .to_string_lossy()
            .into_owned();

        let [_, namespace, name, scheme, _] = file_name
            .split('-')
            .collect::<Vec<&str>>()
            .try_into()
            .map_err(|_| CommitmentParserError::FileNameFormatError)?;

        let commit_data = std::fs::read(p.clone())
            .map_err(|_| CommitmentParserError::ErrorReadingCommitmentFile)?;

        let commit_data: BoundedVec<u8, TableCommitmentMaxLength> =
            BoundedVec::try_from(commit_data)
                .map_err(|_| CommitmentParserError::BoundedVecError)?;

        let commit_data = Some(TableCommitmentBytes { data: commit_data });

        let (dory, hyper_kzg) = match scheme {
            "dynamic_dory" => (commit_data, None),
            "dory" => (commit_data, None),
            "hyper_kzg" => (None, commit_data),
            _ => return Err(CommitmentParserError::InvalidCommitmentScheme),
        };

        let commit = TableCommitmentBytesPerCommitmentScheme {
            dynamic_dory: dory,
            hyper_kzg,
        };

        let namespace = TableNamespace::try_from(namespace.as_bytes().to_vec())
            .map_err(|_| CommitmentParserError::TableNamespaceError)?;
        let name = TableName::try_from(name.as_bytes().to_vec())
            .map_err(|_| CommitmentParserError::TableNameError)?;

        let ident = TableIdentifier { name, namespace };

        Ok(SingleCommit { ident, commit })
    }

    /// Parse commits from a glob string
    pub fn parse_commits_from_glob(
        pattern: &str,
    ) -> Result<
        Vec<(TableIdentifier, TableCommitmentBytesPerCommitmentScheme)>,
        CommitmentParserError,
    > {
        let commits: Vec<(TableIdentifier, TableCommitmentBytesPerCommitmentScheme)> =
            glob::glob(pattern)
                .expect("Failed to read glob pattern")
                .filter_map(Result::ok)
                .map(|pathbuf| {
                    let SingleCommit { ident, commit } =
                        CommitmentParser::single_commit_from_path_buf(pathbuf.clone())
                            .unwrap_or_else(|e| {
                                panic!(
                                    "failed to parse commit for {:?} with error {:?}",
                                    pathbuf, e
                                )
                            });
                    (ident, commit)
                })
                .collect();

        if commits.is_empty() {
            return Err(CommitmentParserError::NoCommitsFoundError);
        }

        Ok(commits)
    }
}

/// Wrapper for a single commit in memory
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo, Serialize, Deserialize,
)]
pub struct SingleCommit {
    ident: TableIdentifier,
    commit: TableCommitmentBytesPerCommitmentScheme,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_commit_from_path_buf_works() {
        let test_file =
            PathBuf::from("testing/-ETHEREUM-CONTRACT_EVT_APPROVALFORALL-dory-commitment.txt");
        let _ = CommitmentParser::single_commit_from_path_buf(test_file).unwrap();
    }

    #[test]
    fn from_glob_works() {
        let res = CommitmentParser::parse_commits_from_glob("testing/*commitment.txt").unwrap();

        assert_eq!(res.len(), 3);
    }
}
