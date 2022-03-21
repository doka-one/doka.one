mod lib;
// use lib::my_lib as denis;
//
// fn my_toto() {
//     denis();
// }

#[cfg(test)]
mod api_customer_tests {
    use crate::lib::test_lib::init;
    use crate::lib::test_lib::close_test;

    #[test]
    fn toto() {

        // my_toto();
        // my_lib();

        init("toto");
        close_test("toto");

        // all_tests::init("toto");
        //denis();
        // all_tests::close_tests("toto");
    }

}