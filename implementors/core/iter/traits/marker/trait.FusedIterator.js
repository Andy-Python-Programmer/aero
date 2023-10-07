(function() {var implementors = {
"aero_kernel":[],
"allocator_api2":[["impl&lt;T, A: <a class=\"trait\" href=\"allocator_api2/alloc/trait.Allocator.html\" title=\"trait allocator_api2::alloc::Allocator\">Allocator</a>&gt; FusedIterator for <a class=\"struct\" href=\"allocator_api2/vec/struct.Drain.html\" title=\"struct allocator_api2::vec::Drain\">Drain</a>&lt;'_, T, A&gt;"],["impl&lt;T, A: <a class=\"trait\" href=\"allocator_api2/alloc/trait.Allocator.html\" title=\"trait allocator_api2::alloc::Allocator\">Allocator</a>&gt; FusedIterator for <a class=\"struct\" href=\"allocator_api2/vec/struct.IntoIter.html\" title=\"struct allocator_api2::vec::IntoIter\">IntoIter</a>&lt;T, A&gt;"],["impl&lt;I: FusedIterator + ?Sized, A: <a class=\"trait\" href=\"allocator_api2/alloc/trait.Allocator.html\" title=\"trait allocator_api2::alloc::Allocator\">Allocator</a>&gt; FusedIterator for <a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;I, A&gt;"]],
"hashbrown":[["impl&lt;K, V, A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_map/struct.Drain.html\" title=\"struct hashbrown::hash_map::Drain\">Drain</a>&lt;'_, K, V, A&gt;"],["impl&lt;K, V, A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_map/struct.IntoKeys.html\" title=\"struct hashbrown::hash_map::IntoKeys\">IntoKeys</a>&lt;K, V, A&gt;"],["impl&lt;K, V&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_map/struct.Keys.html\" title=\"struct hashbrown::hash_map::Keys\">Keys</a>&lt;'_, K, V&gt;"],["impl&lt;K, V, F&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_map/struct.ExtractIf.html\" title=\"struct hashbrown::hash_map::ExtractIf\">ExtractIf</a>&lt;'_, K, V, F&gt;<span class=\"where fmt-newline\">where\n    F: FnMut(&amp;K, &amp;mut V) -&gt; bool,</span>"],["impl&lt;K, V&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_map/struct.IterMut.html\" title=\"struct hashbrown::hash_map::IterMut\">IterMut</a>&lt;'_, K, V&gt;"],["impl&lt;K, V&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_map/struct.Iter.html\" title=\"struct hashbrown::hash_map::Iter\">Iter</a>&lt;'_, K, V&gt;"],["impl&lt;K, V, A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_map/struct.IntoIter.html\" title=\"struct hashbrown::hash_map::IntoIter\">IntoIter</a>&lt;K, V, A&gt;"],["impl&lt;K, F, A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_set/struct.ExtractIf.html\" title=\"struct hashbrown::hash_set::ExtractIf\">ExtractIf</a>&lt;'_, K, F, A&gt;<span class=\"where fmt-newline\">where\n    F: FnMut(&amp;K) -&gt; bool,</span>"],["impl&lt;T, S, A&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_set/struct.Union.html\" title=\"struct hashbrown::hash_set::Union\">Union</a>&lt;'_, T, S, A&gt;<span class=\"where fmt-newline\">where\n    T: Eq + Hash,\n    S: BuildHasher,\n    A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>,</span>"],["impl&lt;K, A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_set/struct.IntoIter.html\" title=\"struct hashbrown::hash_set::IntoIter\">IntoIter</a>&lt;K, A&gt;"],["impl&lt;K, V&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_map/struct.ValuesMut.html\" title=\"struct hashbrown::hash_map::ValuesMut\">ValuesMut</a>&lt;'_, K, V&gt;"],["impl&lt;K, V, A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_map/struct.IntoValues.html\" title=\"struct hashbrown::hash_map::IntoValues\">IntoValues</a>&lt;K, V, A&gt;"],["impl&lt;K&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_set/struct.Iter.html\" title=\"struct hashbrown::hash_set::Iter\">Iter</a>&lt;'_, K&gt;"],["impl&lt;K, A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_set/struct.Drain.html\" title=\"struct hashbrown::hash_set::Drain\">Drain</a>&lt;'_, K, A&gt;"],["impl&lt;T, S, A&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_set/struct.SymmetricDifference.html\" title=\"struct hashbrown::hash_set::SymmetricDifference\">SymmetricDifference</a>&lt;'_, T, S, A&gt;<span class=\"where fmt-newline\">where\n    T: Eq + Hash,\n    S: BuildHasher,\n    A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>,</span>"],["impl&lt;K, V&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_map/struct.Values.html\" title=\"struct hashbrown::hash_map::Values\">Values</a>&lt;'_, K, V&gt;"],["impl&lt;T, S, A&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_set/struct.Difference.html\" title=\"struct hashbrown::hash_set::Difference\">Difference</a>&lt;'_, T, S, A&gt;<span class=\"where fmt-newline\">where\n    T: Eq + Hash,\n    S: BuildHasher,\n    A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>,</span>"],["impl&lt;T, S, A&gt; FusedIterator for <a class=\"struct\" href=\"hashbrown/hash_set/struct.Intersection.html\" title=\"struct hashbrown::hash_set::Intersection\">Intersection</a>&lt;'_, T, S, A&gt;<span class=\"where fmt-newline\">where\n    T: Eq + Hash,\n    S: BuildHasher,\n    A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>,</span>"]],
"lru":[["impl&lt;K, V&gt; FusedIterator for <a class=\"struct\" href=\"lru/struct.IntoIter.html\" title=\"struct lru::IntoIter\">IntoIter</a>&lt;K, V&gt;<span class=\"where fmt-newline\">where\n    K: Hash + Eq,</span>"],["impl&lt;'a, K, V&gt; FusedIterator for <a class=\"struct\" href=\"lru/struct.IterMut.html\" title=\"struct lru::IterMut\">IterMut</a>&lt;'a, K, V&gt;"],["impl&lt;'a, K, V&gt; FusedIterator for <a class=\"struct\" href=\"lru/struct.Iter.html\" title=\"struct lru::Iter\">Iter</a>&lt;'a, K, V&gt;"]],
"serde_json":[["impl&lt;'de, R, T&gt; FusedIterator for <a class=\"struct\" href=\"serde_json/struct.StreamDeserializer.html\" title=\"struct serde_json::StreamDeserializer\">StreamDeserializer</a>&lt;'de, R, T&gt;<span class=\"where fmt-newline\">where\n    R: <a class=\"trait\" href=\"serde_json/de/trait.Read.html\" title=\"trait serde_json::de::Read\">Read</a>&lt;'de&gt; + Fused,\n    T: <a class=\"trait\" href=\"serde/de/trait.Deserialize.html\" title=\"trait serde::de::Deserialize\">Deserialize</a>&lt;'de&gt;,</span>"],["impl&lt;'a&gt; FusedIterator for <a class=\"struct\" href=\"serde_json/map/struct.IterMut.html\" title=\"struct serde_json::map::IterMut\">IterMut</a>&lt;'a&gt;"],["impl&lt;'a&gt; FusedIterator for <a class=\"struct\" href=\"serde_json/map/struct.Iter.html\" title=\"struct serde_json::map::Iter\">Iter</a>&lt;'a&gt;"],["impl&lt;'a&gt; FusedIterator for <a class=\"struct\" href=\"serde_json/map/struct.Keys.html\" title=\"struct serde_json::map::Keys\">Keys</a>&lt;'a&gt;"],["impl&lt;'a&gt; FusedIterator for <a class=\"struct\" href=\"serde_json/map/struct.ValuesMut.html\" title=\"struct serde_json::map::ValuesMut\">ValuesMut</a>&lt;'a&gt;"],["impl&lt;'a&gt; FusedIterator for <a class=\"struct\" href=\"serde_json/map/struct.Values.html\" title=\"struct serde_json::map::Values\">Values</a>&lt;'a&gt;"],["impl FusedIterator for <a class=\"struct\" href=\"serde_json/map/struct.IntoIter.html\" title=\"struct serde_json::map::IntoIter\">IntoIter</a>"]]
};if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()