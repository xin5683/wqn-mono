use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PatchField<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<'de, T> Deserialize<'de> for PatchField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

impl<T> PatchField<T> {
    pub fn as_nullable_value(&self) -> Option<&T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Missing | Self::Null => None,
        }
    }

    pub fn into_nullable(self) -> (bool, Option<T>) {
        match self {
            Self::Missing => (false, None),
            Self::Null => (true, None),
            Self::Value(value) => (true, Some(value)),
        }
    }
}

impl<T> PatchField<Vec<T>> {
    pub fn as_slice_or_empty(&self) -> &[T] {
        match self {
            Self::Value(value) => value.as_slice(),
            Self::Missing | Self::Null => &[],
        }
    }

    pub fn into_vec_or_empty(self) -> (bool, Option<Vec<T>>) {
        match self {
            Self::Missing => (false, None),
            Self::Null => (true, Some(Vec::new())),
            Self::Value(value) => (true, Some(value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, serde::Deserialize, PartialEq)]
    #[serde(default)]
    struct Wrap {
        name: PatchField<String>,
        tags: PatchField<Vec<String>>,
        count: PatchField<i64>,
    }

    #[test]
    fn omitted_field_is_missing() {
        // `#[serde(default)]` makes an absent field use `Default = Missing`,
        // mirroring how production PATCH DTOs declare their fields. Without it,
        // serde would route the absent field through `Deserialize` and yield `Null`.
        let parsed: Wrap = serde_json::from_str(r#"{"count": 7}"#).unwrap();
        assert_eq!(parsed.name, PatchField::Missing);
        assert_eq!(parsed.tags, PatchField::Missing);
    }

    #[test]
    fn explicit_null_is_null_not_missing() {
        let parsed: Wrap =
            serde_json::from_str(r#"{"name": null, "tags": null}"#).unwrap();
        assert_eq!(parsed.name, PatchField::Null);
        assert_eq!(parsed.tags, PatchField::Null);
    }

    #[test]
    fn present_value_is_kept() {
        let parsed: Wrap =
            serde_json::from_str(r#"{"name": "x", "tags": ["a"], "count": 3}"#).unwrap();
        assert_eq!(parsed.name, PatchField::Value("x".to_owned()));
        assert_eq!(parsed.tags, PatchField::Value(vec!["a".to_owned()]));
        assert_eq!(parsed.count, PatchField::Value(3));
    }

    #[test]
    fn default_is_missing() {
        assert_eq!(PatchField::<String>::default(), PatchField::Missing);
    }

    #[test]
    fn as_nullable_value_only_returns_present_value() {
        assert_eq!(PatchField::Value(5_i64).as_nullable_value(), Some(&5));
        assert_eq!(PatchField::<i64>::Null.as_nullable_value(), None);
        assert_eq!(PatchField::<i64>::Missing.as_nullable_value(), None);
    }

    #[test]
    fn into_nullable_distinguishes_all_three_states() {
        assert_eq!(PatchField::<i64>::Missing.into_nullable(), (false, None));
        assert_eq!(PatchField::<i64>::Null.into_nullable(), (true, None));
        assert_eq!(PatchField::Value(5_i64).into_nullable(), (true, Some(5)));
    }

    #[test]
    fn slice_helpers_treat_missing_and_null_as_empty() {
        let value = PatchField::Value(vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(
            value.as_slice_or_empty().to_vec(),
            vec!["a".to_owned(), "b".to_owned()]
        );
        assert!(PatchField::<Vec<String>>::Null.as_slice_or_empty().is_empty());
        assert!(PatchField::<Vec<String>>::Missing.as_slice_or_empty().is_empty());
    }

    #[test]
    fn into_vec_or_empty_signals_write_with_empty_for_null() {
        assert_eq!(
            PatchField::<Vec<String>>::Missing.into_vec_or_empty(),
            (false, None)
        );
        // null means "write the field" with an empty collection
        assert_eq!(
            PatchField::<Vec<String>>::Null.into_vec_or_empty(),
            (true, Some(Vec::new()))
        );
        assert_eq!(
            PatchField::Value(vec!["a".to_owned()]).into_vec_or_empty(),
            (true, Some(vec!["a".to_owned()]))
        );
    }
}
