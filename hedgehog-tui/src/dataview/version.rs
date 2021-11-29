#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(transparent)]
pub(crate) struct Version(usize);

impl Version {
    fn advanced(&self) -> Version {
        Version(self.0.wrapping_add(1))
    }
}

impl Default for Version {
    fn default() -> Self {
        Version(0)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Versioned<T>(Version, T);

impl<T> Versioned<T> {
    pub(crate) fn new(value: T) -> Self {
        Versioned(Version::default(), value)
    }

    pub(crate) fn with_version(mut self, version: Version) -> Self {
        self.0 = version;
        self
    }

    pub(crate) fn update<R>(&self, new_value: R) -> Versioned<R> {
        Versioned(self.0.advanced(), new_value)
    }

    pub(crate) fn with_data<R>(&self, new_value: R) -> Versioned<R> {
        Versioned(self.0, new_value)
    }

    pub(crate) fn same_version<R>(&self, other: &Versioned<R>) -> bool {
        self.0 == other.0
    }

    pub(crate) fn as_ref(&self) -> Versioned<&T> {
        Versioned(self.0, &self.1)
    }

    pub(crate) fn map<R>(self, f: impl FnOnce(T) -> R) -> Versioned<R> {
        Versioned(self.0, f(self.1))
    }

    pub(crate) fn as_inner(&self) -> &T {
        &self.1
    }

    pub(crate) fn version(&self) -> Version {
        self.0
    }

    pub(crate) fn into_inner(self) -> T {
        self.1
    }

    pub(crate) fn deconstruct(self) -> (Version, T) {
        (self.0, self.1)
    }
}

impl<T> Versioned<Option<T>> {
    pub fn take(&mut self) -> Option<T> {
        self.1.take()
    }
}
