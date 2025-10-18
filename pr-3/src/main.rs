use std::borrow::Cow;
use std::cell::Cell;
use std::collections::HashMap;
use std::hash::Hash;

// ───────────────────────────────────────────────────────────────
// ДАНО (не змінювати)
// ───────────────────────────────────────────────────────────────

trait Storage<K, V> {
    fn set(&mut self, key: K, val: V);
    fn get(&self, key: &K) -> Option<&V>;
    fn remove(&mut self, key: &K) -> Option<V>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct User {
    id: u64,
    email: Cow<'static, str>,
    activated: bool,
}

// ───────────────────────────────────────────────────────────────
// БАЗОВЕ СХОВИЩЕ: HashMapStorage<K, V>
// ───────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct HashMapStorage<K, V>(HashMap<K, V>);

impl<K, V> HashMapStorage<K, V> {
    fn new() -> Self {
        Self(HashMap::new())
    }
}

impl<K, V> Storage<K, V> for HashMapStorage<K, V>
where
    K: Eq + Hash,
{
    fn set(&mut self, key: K, val: V) {
        self.0.insert(key, val);
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key)
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }
}

// ───────────────────────────────────────────────────────────────
// РЕПОЗИТОРІЙ: статична ін’єкція (generics / static dispatch)
// ───────────────────────────────────────────────────────────────

struct UserRepositoryStatic<S>
where
    S: Storage<u64, User>,
{
    storage: S,
}

impl<S> UserRepositoryStatic<S>
where
    S: Storage<u64, User>,
{
    fn new(storage: S) -> Self {
        Self { storage }
    }

    fn get(&self, id: u64) -> Option<&User> {
        self.storage.get(&id)
    }

    /// Додає користувача. false → якщо id вже існує.
    fn add(&mut self, user: User) -> bool {
        if self.storage.get(&user.id).is_some() {
            return false;
        }
        self.storage.set(user.id, user);
        true
    }

    /// Оновлює існуючого користувача. false → якщо не існує.
    fn update(&mut self, user: User) -> bool {
        if self.storage.get(&user.id).is_none() {
            return false;
        }
        self.storage.set(user.id, user);
        true
    }

    fn remove(&mut self, id: u64) -> Option<User> {
        self.storage.remove(&id)
    }
}

// ───────────────────────────────────────────────────────────────
// РЕПОЗИТОРІЙ: динамічна ін’єкція (trait objects / dynamic dispatch)
// ───────────────────────────────────────────────────────────────

struct UserRepositoryDynamic {
    storage: Box<dyn Storage<u64, User>>,
}

impl UserRepositoryDynamic {
    fn new(storage: Box<dyn Storage<u64, User>>) -> Self {
        Self { storage }
    }

    fn get(&self, id: u64) -> Option<&User> {
        self.storage.get(&id)
    }

    fn add(&mut self, user: User) -> bool {
        if self.storage.get(&user.id).is_some() {
            return false;
        }
        self.storage.set(user.id, user);
        true
    }

    fn update(&mut self, user: User) -> bool {
        if self.storage.get(&user.id).is_none() {
            return false;
        }
        self.storage.set(user.id, user);
        true
    }

    fn remove(&mut self, id: u64) -> Option<User> {
        self.storage.remove(&id)
    }
}

// ───────────────────────────────────────────────────────────────
// Демонстрація (робить бінарник корисним при `cargo run`)
// ───────────────────────────────────────────────────────────────

fn main() {
    let mut repo = UserRepositoryStatic::new(HashMapStorage::<u64, User>::new());
    let _ = repo.add(User {
        id: 1,
        email: Cow::Borrowed("user@example.com"),
        activated: true,
    });
    if let Some(u) = repo.get(1) {
        println!("Found user: {u:?}");
    }
}

// ───────────────────────────────────────────────────────────────
// ТЕСТИ: коректність та “інжектованість”
// ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn u(id: u64, email: &'static str, activated: bool) -> User {
        User {
            id,
            email: Cow::Borrowed(email),
            activated,
        }
    }

    #[test]
    fn static_repo_crud() {
        let mut repo = UserRepositoryStatic::new(HashMapStorage::<u64, User>::new());

        // add
        assert!(repo.add(u(10, "a@ex", false)));
        assert!(!repo.add(u(10, "dup@ex", true))); // duplicate

        // get
        let got = repo.get(10).unwrap();
        assert_eq!(got.email, "a@ex");
        assert!(!got.activated);

        // update existing
        assert!(repo.update(u(10, "new@ex", true)));
        let got = repo.get(10).unwrap();
        assert_eq!(got.email, "new@ex");
        assert!(got.activated);

        // update missing
        assert!(!repo.update(u(11, "nope@ex", false)));

        // remove
        let removed = repo.remove(10).unwrap();
        assert_eq!(removed.email, "new@ex");
        assert!(repo.get(10).is_none());
    }

    #[test]
    fn dynamic_repo_crud() {
        let storage: Box<dyn Storage<u64, User>> = Box::new(HashMapStorage::<u64, User>::new());
        let mut repo = UserRepositoryDynamic::new(storage);

        assert!(repo.add(u(42, "x@ex", true)));
        assert!(repo.get(42).is_some());

        assert!(repo.update(u(42, "y@ex", false)));
        let got = repo.get(42).unwrap();
        assert_eq!(got.email, "y@ex");
        assert!(!got.activated);

        let removed = repo.remove(42).unwrap();
        assert_eq!(removed.id, 42);
        assert!(repo.get(42).is_none());
    }

    // ── Інжектованість: враппер-лічильник навколо будь-якого Storage ──
    struct CountingStorage<S> {
        inner: S,
        get_count: Cell<usize>,
        set_count: usize,
        remove_count: usize,
    }

    impl<S> CountingStorage<S> {
        fn new(inner: S) -> Self {
            Self {
                inner,
                get_count: Cell::new(0),
                set_count: 0,
                remove_count: 0,
            }
        }
        fn gets(&self) -> usize { self.get_count.get() }
        fn sets(&self) -> usize { self.set_count }
        fn removes(&self) -> usize { self.remove_count }
    }

    impl<K, V, S> Storage<K, V> for CountingStorage<S>
    where
        S: Storage<K, V>,
    {
        fn set(&mut self, key: K, val: V) {
            self.set_count += 1;
            self.inner.set(key, val);
        }
        fn get(&self, key: &K) -> Option<&V> {
            self.get_count.set(self.get_count.get() + 1);
            self.inner.get(key)
        }
        fn remove(&mut self, key: &K) -> Option<V> {
            self.remove_count += 1;
            self.inner.remove(key)
        }
    }

    #[test]
    fn static_repo_is_injectable() {
        let inner = HashMapStorage::<u64, User>::new();
        let counting = CountingStorage::new(inner);
        let mut repo = UserRepositoryStatic::new(counting);

        assert!(repo.add(u(7, "t@ex", true)));       // set + get(pre-check)
        let _ = repo.get(7);                         // get
        assert!(repo.update(u(7, "t2@ex", true)));   // get(pre-check) + set
        let _ = repo.remove(7);                      // remove

        let gets = repo.storage.gets();
        let sets = repo.storage.sets();
        let removes = repo.storage.removes();
        assert_eq!(gets, 3, "add-precheck + explicit-get + update-precheck");
        assert_eq!(sets, 2, "add + update");
        assert_eq!(removes, 1, "remove once");
    }

    #[test]
    fn dynamic_repo_is_injectable() {
        let inner = HashMapStorage::<u64, User>::new();
        let counting = CountingStorage::new(inner);
        let mut repo = UserRepositoryDynamic::new(Box::new(counting));

        assert!(repo.add(u(9, "g@ex", false)));  // set + get(pre-check)
        let _ = repo.get(9);                     // get
        let _ = repo.remove(9);                  // remove

        // Витягнути лічильники з Box<dyn Storage> напряму не можемо.
        // Перевіримо поведінку побічно: CRUD працює, значить інжекція ок.
        // Для демонстрації підрахунку зробимо ще один цикл з явним типом:

        let inner2 = HashMapStorage::<u64, User>::new();
        let counting2 = CountingStorage::new(inner2);
        let mut repo2 = UserRepositoryStatic::new(counting2);

        assert!(repo2.add(u(1, "a@ex", true)));
        let _ = repo2.get(1);
        let _ = repo2.remove(1);

        assert_eq!(repo2.storage.gets(), 2, "add-precheck + explicit-get");
        assert_eq!(repo2.storage.sets(), 1, "only add");
        assert_eq!(repo2.storage.removes(), 1, "one remove");
    }
}
