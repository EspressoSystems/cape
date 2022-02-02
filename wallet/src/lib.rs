pub mod cli_client;
pub mod disco;
pub mod mocks;
pub mod routes;
pub mod wallet;
pub mod web;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
