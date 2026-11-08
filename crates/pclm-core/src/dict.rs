//! .NET `Dictionary<TKey, TValue>` 의 차례를 그대로 따르는 사전.
//!
//! C# 의 사전은 넣은 차례로 돌지만 **지운 자리를 다음에 넣는 것이 다시 쓴다**(빈자리 목록은 나중에 빈 것부터,
//! `Clear` 는 처음으로). 뷰 짓기가 이 차례에 기댄다 — `v_맞춤뷰` 의 `CASE` 갈래 차례는 맞춤 뷰를 지어 보며
//! 넣고 지운 사전을 그대로 돈 것이라, 차례가 갈리면 `sqlite_master` 의 글이 C# 과 갈린다.

/// 넣은 차례 + 빈자리 다시 쓰기. 열쇠는 `==` 로 견준다(작아서 훑는다).
#[derive(Clone, Debug, PartialEq)]
pub struct Dict<K, V> {
    entries: Vec<Option<(K, V)>>,
    free: Vec<usize>,
}

impl<K, V> Default for Dict<K, V> {
    fn default() -> Self {
        Dict {
            entries: Vec::new(),
            free: Vec::new(),
        }
    }
}

impl<K: PartialEq, V> Dict<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    fn position(&self, key: &K) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.as_ref().is_some_and(|(k, _)| k == key))
    }

    /// `dict[key] = value` — 있으면 그 자리에서 값만 바꾸고, 없으면 마지막에 빈 자리(없으면 끝)에 넣는다.
    pub fn insert(&mut self, key: K, value: V) {
        if let Some(i) = self.position(&key) {
            self.entries[i] = Some((key, value));
        } else if let Some(i) = self.free.pop() {
            self.entries[i] = Some((key, value));
        } else {
            self.entries.push(Some((key, value)));
        }
    }

    /// `Remove`.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let i = self.position(key)?;
        self.free.push(i);
        self.entries[i].take().map(|(_, v)| v)
    }

    /// `Clear`.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.free.clear();
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.position(key)
            .and_then(|i| self.entries[i].as_ref().map(|(_, v)| v))
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.position(key).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// C# 의 `foreach` 차례.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().flatten().map(|(k, v)| (k, v))
    }

    /// 차례 그대로 쌍으로.
    pub fn into_vec(self) -> Vec<(K, V)> {
        self.entries.into_iter().flatten().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_slots_are_reused_last_freed_first() {
        let mut d = Dict::new();
        for k in ["a", "b", "c", "d"] {
            d.insert(k, 0);
        }
        d.remove(&"b");
        d.remove(&"c");
        d.insert("x", 1); // c 의 자리
        d.insert("y", 2); // b 의 자리
        d.insert("a", 9); // 제자리
        let keys: Vec<_> = d.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, ["a", "y", "x", "d"]);
        d.clear();
        d.insert("z", 0);
        assert_eq!(d.into_vec(), [("z", 0)]);
    }
}
