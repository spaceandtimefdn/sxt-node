# Transaction Node Data Loader

This library acts as the bootstrap data loader for transaction nodes.

## How to Use the Library

One public function is exposed:

### Functions

- **`run_data_loader(base_path, max_retries, delay)`**: Loads data from the specified base path with configurable retries and delays.

## Environment Variables

The following environment variables are needed to run this library:


- **`AZURE_ACCOUNT_NAME`**: Azure account access key.
- **`AZURE_ACCOUNT_NAME`**: Azure account name.
- **`AZURE_CONTAINER_NAME`**: Azure container name.
- **`AZURE_BASE_PATH`**: Base path for the Azure container.
- **`AZURE_ENDPOINT`**: Endpoint of the public azure repo.
- **`DATABASE_URL`**: Local Postgres database URL.
- **` PROCESS_ONLY_HEAD `** Use it only when testing. It will process only 1 file from each directory.

### Example Usage

```rust
use data_loader::run_data_loader;
use std::{time::Duration}

let base_path = env::var("BASE_PATH").map_err(|_| "Missing env variable BASE_PATH")?;
run_data_loader(&base_path, 2, Duration::new(2, 0)))?

