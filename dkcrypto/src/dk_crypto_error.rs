// use std::error;
// use std::fmt;
//
// // Define our error types. These may be customized for our error handling cases.
// // Now we will be able to write our own errors, defer to an underlying error
// // implementation, or do something in between.
// #[derive(Debug, Clone)]
// pub struct DkCryptoError;
//
// // Generation of an error is completely separate from how it is displayed.
// // There's no need to be concerned about cluttering complex logic with the display style.
// //
// // Note that we don't store any extra info about the errors. This means we can't state
// // which string failed to parse without modifying our types to carry that information.
// impl fmt::Display for DkCryptoError {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "misssing or invalid negotiated price for the contract")
//     }
// }
//
// // This is important for other errors to wrap this one.
// impl error::Error for DkCryptoError {
//     fn source(&self) -> Option<&(dyn error::Error + 'static)> {
//         // Generic error, underlying cause isn't tracked.
//         None
//     }
// }