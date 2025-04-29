use std::path::PathBuf;

use rand::Rng;

/// A small test utility for creating a directory for use in a test that gets automatically
/// deleted at the end of the test or in the event of a failure by `Drop`.
pub struct TestDirectory {
    /// The path of the created test directory.
    pub path: PathBuf,
}

impl TestDirectory {
    /// Generate a random directory for this test.
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        let sub_directory_name: u128 = rng.gen();

        let path = format!("test_directory/{sub_directory_name}")
            .parse()
            .unwrap();

        std::fs::create_dir_all(&path).unwrap();

        TestDirectory { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).unwrap();
    }
}
