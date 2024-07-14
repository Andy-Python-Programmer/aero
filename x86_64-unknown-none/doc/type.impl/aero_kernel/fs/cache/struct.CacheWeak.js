(function() {var type_impls = {
"aero_kernel":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-CacheWeak%3CT%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/aero_kernel/fs/cache.rs.html#107-121\">source</a><a href=\"#impl-CacheWeak%3CT%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T: <a class=\"trait\" href=\"aero_kernel/fs/cache/trait.CacheDropper.html\" title=\"trait aero_kernel::fs::cache::CacheDropper\">CacheDropper</a>&gt; <a class=\"struct\" href=\"aero_kernel/fs/cache/struct.CacheWeak.html\" title=\"struct aero_kernel::fs::cache::CacheWeak\">CacheWeak</a>&lt;T&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.new\" class=\"method\"><a class=\"src rightside\" href=\"src/aero_kernel/fs/cache.rs.html#110-112\">source</a><h4 class=\"code-header\">pub fn <a href=\"aero_kernel/fs/cache/struct.CacheWeak.html#tymethod.new\" class=\"fn\">new</a>() -&gt; Self</h4></section></summary><div class=\"docblock\"><p>Constructs a new <code>Weak&lt;T&gt;</code>, without allocating any memory.\nCalling [<code>upgrade</code>] on the return value always gives <a href=\"aero_kernel/prelude/rust_2021/_core/option/enum.Option.html#variant.None\" title=\"variant aero_kernel::prelude::rust_2021::_core::option::Option::None\"><code>None</code></a>.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.upgrade\" class=\"method\"><a class=\"src rightside\" href=\"src/aero_kernel/fs/cache.rs.html#118-120\">source</a><h4 class=\"code-header\">pub fn <a href=\"aero_kernel/fs/cache/struct.CacheWeak.html#tymethod.upgrade\" class=\"fn\">upgrade</a>(&amp;self) -&gt; <a class=\"enum\" href=\"aero_kernel/prelude/rust_2021/_core/option/enum.Option.html\" title=\"enum aero_kernel::prelude::rust_2021::_core::option::Option\">Option</a>&lt;<a class=\"struct\" href=\"aero_kernel/fs/cache/struct.CacheArc.html\" title=\"struct aero_kernel::fs::cache::CacheArc\">CacheArc</a>&lt;T&gt;&gt;</h4></section></summary><div class=\"docblock\"><p>Attempts to upgrade the Weak pointer to an Arc, delaying dropping of the inner\nvalue if successful.</p>\n<p>Returns <a href=\"aero_kernel/prelude/rust_2021/_core/option/enum.Option.html#variant.None\" title=\"variant aero_kernel::prelude::rust_2021::_core::option::Option::None\"><code>None</code></a> if the inner value has since been dropped.</p>\n</div></details></div></details>",0,"aero_kernel::fs::cache::INodeCacheWeakItem"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Clone-for-CacheWeak%3CT%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/aero_kernel/fs/cache.rs.html#123-129\">source</a><a href=\"#impl-Clone-for-CacheWeak%3CT%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T: <a class=\"trait\" href=\"aero_kernel/fs/cache/trait.CacheDropper.html\" title=\"trait aero_kernel::fs::cache::CacheDropper\">CacheDropper</a>&gt; <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/clone/trait.Clone.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::clone::Clone\">Clone</a> for <a class=\"struct\" href=\"aero_kernel/fs/cache/struct.CacheWeak.html\" title=\"struct aero_kernel::fs::cache::CacheWeak\">CacheWeak</a>&lt;T&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.clone\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/aero_kernel/fs/cache.rs.html#126-128\">source</a><a href=\"#method.clone\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"aero_kernel/prelude/rust_2021/_core/clone/trait.Clone.html#tymethod.clone\" class=\"fn\">clone</a>(&amp;self) -&gt; Self</h4></section></summary><div class=\"docblock\"><p>Makes a clone of the <code>CacheWeak&lt;T&gt;</code> pointer.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.clone_from\" class=\"method trait-impl\"><span class=\"since rightside\" title=\"Stable since Rust version 1.0.0\">1.0.0</span><a href=\"#method.clone_from\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"aero_kernel/prelude/rust_2021/_core/clone/trait.Clone.html#method.clone_from\" class=\"fn\">clone_from</a>(&amp;mut self, source: &amp;Self)</h4></section></summary><div class='docblock'>Performs copy-assignment from <code>source</code>. <a href=\"aero_kernel/prelude/rust_2021/_core/clone/trait.Clone.html#method.clone_from\">Read more</a></div></details></div></details>","Clone","aero_kernel::fs::cache::INodeCacheWeakItem"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Default-for-CacheWeak%3CT%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/aero_kernel/fs/cache.rs.html#131-135\">source</a><a href=\"#impl-Default-for-CacheWeak%3CT%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T: <a class=\"trait\" href=\"aero_kernel/fs/cache/trait.CacheDropper.html\" title=\"trait aero_kernel::fs::cache::CacheDropper\">CacheDropper</a>&gt; <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/default/trait.Default.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::default::Default\">Default</a> for <a class=\"struct\" href=\"aero_kernel/fs/cache/struct.CacheWeak.html\" title=\"struct aero_kernel::fs::cache::CacheWeak\">CacheWeak</a>&lt;T&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.default\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/aero_kernel/fs/cache.rs.html#132-134\">source</a><a href=\"#method.default\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"aero_kernel/prelude/rust_2021/_core/default/trait.Default.html#tymethod.default\" class=\"fn\">default</a>() -&gt; Self</h4></section></summary><div class='docblock'>Returns the “default value” for a type. <a href=\"aero_kernel/prelude/rust_2021/_core/default/trait.Default.html#tymethod.default\">Read more</a></div></details></div></details>","Default","aero_kernel::fs::cache::INodeCacheWeakItem"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()