macro_rules! filter_unwrap {
	($list:expr, $pat:path) => {
		$list
			.into_iter()
			.filter_map(|item| if let $pat(a) = item { Some(a) } else { None })
	};
}
pub(crate) use filter_unwrap;

macro_rules! fallible_quote {
	($($tt:tt)*) => {
		syn::parse::Parse::parse.parse2(quote::quote! { $($tt)* }).map_err(|e| {
			syn::parse::Error::new(
				e.span(),
				format!("Internal error: {} at file {}:{} - this is likely a bug", e, file!(), line!()),
			)
		})
	};
}

pub(crate) use fallible_quote;

macro_rules! map_or_bail {
	($iter:expr, $closure:expr) => {{
		let results: ::std::result::Result<Vec<_>, _> = $iter.into_iter().map($closure).collect();
		results?
	}};
}

pub(crate) use map_or_bail;
