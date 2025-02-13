// use derive_more::From;

// pub type Result<T> = core::result::Result<T, Error>;

// #[derive(Debug, From)]
// pub enum Error {
//     PathNotFound,

//     #[from]
//     Custom(String),
//     // #[from]
//     // Files(crate::files::error::Error),
// }

// impl From<&str> for Error {
//     fn from(value: &str) -> Self {
//         Self::Custom(value.to_string())
//     }
// }

// impl core::fmt::Display for Error {
//     fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
//         write!(fmt, "{self:?}")
//     }
// }

// impl std::error::Error for Error {}
