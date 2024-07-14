(function() {var type_impls = {
"aero_kernel":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Extend%3C(A,+B)%3E-for-(ExtendA,+ExtendB)\" class=\"impl\"><span class=\"since rightside\" title=\"Stable since Rust version 1.56.0\">1.56.0</span><a href=\"#impl-Extend%3C(A,+B)%3E-for-(ExtendA,+ExtendB)\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;A, B, ExtendA, ExtendB&gt; <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Extend.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::iter::Extend\">Extend</a>&lt;(A, B)&gt; for (ExtendA, ExtendB)<div class=\"where\">where\n    ExtendA: <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Extend.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::iter::Extend\">Extend</a>&lt;A&gt;,\n    ExtendB: <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Extend.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::iter::Extend\">Extend</a>&lt;B&gt;,</div></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.extend\" class=\"method trait-impl\"><a href=\"#method.extend\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Extend.html#tymethod.extend\" class=\"fn\">extend</a>&lt;T&gt;(&amp;mut self, into_iter: T)<div class=\"where\">where\n    T: <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.IntoIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::iter::IntoIterator\">IntoIterator</a>&lt;Item = (A, B)&gt;,</div></h4></section></summary><div class=\"docblock\"><p>Allows to <code>extend</code> a tuple of collections that also implement <code>Extend</code>.</p>\n<p>See also: <a href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Iterator.html#method.unzip\" title=\"method aero_kernel::prelude::rust_2021::_core::iter::Iterator::unzip\"><code>Iterator::unzip</code></a></p>\n<h5 id=\"examples\"><a class=\"doc-anchor\" href=\"#examples\">§</a>Examples</h5>\n<div class=\"example-wrap\"><pre class=\"rust rust-example-rendered\"><code><span class=\"kw\">let </span><span class=\"kw-2\">mut </span>tuple = (<span class=\"macro\">vec!</span>[<span class=\"number\">0</span>], <span class=\"macro\">vec!</span>[<span class=\"number\">1</span>]);\ntuple.extend([(<span class=\"number\">2</span>, <span class=\"number\">3</span>), (<span class=\"number\">4</span>, <span class=\"number\">5</span>), (<span class=\"number\">6</span>, <span class=\"number\">7</span>)]);\n<span class=\"macro\">assert_eq!</span>(tuple.<span class=\"number\">0</span>, [<span class=\"number\">0</span>, <span class=\"number\">2</span>, <span class=\"number\">4</span>, <span class=\"number\">6</span>]);\n<span class=\"macro\">assert_eq!</span>(tuple.<span class=\"number\">1</span>, [<span class=\"number\">1</span>, <span class=\"number\">3</span>, <span class=\"number\">5</span>, <span class=\"number\">7</span>]);\n\n<span class=\"comment\">// also allows for arbitrarily nested tuples as elements\n</span><span class=\"kw\">let </span><span class=\"kw-2\">mut </span>nested_tuple = (<span class=\"macro\">vec!</span>[<span class=\"number\">1</span>], (<span class=\"macro\">vec!</span>[<span class=\"number\">2</span>], <span class=\"macro\">vec!</span>[<span class=\"number\">3</span>]));\nnested_tuple.extend([(<span class=\"number\">4</span>, (<span class=\"number\">5</span>, <span class=\"number\">6</span>)), (<span class=\"number\">7</span>, (<span class=\"number\">8</span>, <span class=\"number\">9</span>))]);\n\n<span class=\"kw\">let </span>(a, (b, c)) = nested_tuple;\n<span class=\"macro\">assert_eq!</span>(a, [<span class=\"number\">1</span>, <span class=\"number\">4</span>, <span class=\"number\">7</span>]);\n<span class=\"macro\">assert_eq!</span>(b, [<span class=\"number\">2</span>, <span class=\"number\">5</span>, <span class=\"number\">8</span>]);\n<span class=\"macro\">assert_eq!</span>(c, [<span class=\"number\">3</span>, <span class=\"number\">6</span>, <span class=\"number\">9</span>]);</code></pre></div>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.extend_one\" class=\"method trait-impl\"><a href=\"#method.extend_one\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Extend.html#method.extend_one\" class=\"fn\">extend_one</a>(&amp;mut self, item: (A, B))</h4></section></summary><span class=\"item-info\"><div class=\"stab unstable\"><span class=\"emoji\">🔬</span><span>This is a nightly-only experimental API. (<code>extend_one</code>)</span></div></span><div class='docblock'>Extends a collection with exactly one element.</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.extend_reserve\" class=\"method trait-impl\"><a href=\"#method.extend_reserve\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Extend.html#method.extend_reserve\" class=\"fn\">extend_reserve</a>(&amp;mut self, additional: usize)</h4></section></summary><span class=\"item-info\"><div class=\"stab unstable\"><span class=\"emoji\">🔬</span><span>This is a nightly-only experimental API. (<code>extend_one</code>)</span></div></span><div class='docblock'>Reserves capacity in a collection for the given number of additional elements. <a href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Extend.html#method.extend_reserve\">Read more</a></div></details></div></details>","Extend<(A, B)>","aero_kernel::arch::x86_64::tls::ProcFsCpuFeature","aero_kernel::fs::block::PageCacheKey","aero_kernel::fs::cache::INodeCacheKey","aero_kernel::fs::cache::DirCacheKey","aero_kernel::fs::MountKey"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-From%3C%5BT;+2%5D%3E-for-(T,+T)\" class=\"impl\"><span class=\"since rightside\" title=\"Stable since Rust version 1.71.0\">1.71.0</span><a href=\"#impl-From%3C%5BT;+2%5D%3E-for-(T,+T)\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T&gt; <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/convert/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::convert::From\">From</a>&lt;[T; 2]&gt; for (T, T)</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.from\" class=\"method trait-impl\"><a href=\"#method.from\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"aero_kernel/prelude/rust_2021/_core/convert/trait.From.html#tymethod.from\" class=\"fn\">from</a>(array: [T; 2]) -&gt; (T, T)</h4></section></summary><div class='docblock'>Converts to this type from the input type.</div></details></div></details>","From<[T; 2]>","aero_kernel::arch::x86_64::tls::ProcFsCpuFeature","aero_kernel::fs::block::PageCacheKey","aero_kernel::fs::cache::INodeCacheKey","aero_kernel::fs::cache::DirCacheKey","aero_kernel::fs::MountKey"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-FromIterator%3C(AE,+BE)%3E-for-(A,+B)\" class=\"impl\"><span class=\"since rightside\" title=\"Stable since Rust version 1.79.0\">1.79.0</span><a href=\"#impl-FromIterator%3C(AE,+BE)%3E-for-(A,+B)\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;A, B, AE, BE&gt; <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::iter::FromIterator\">FromIterator</a>&lt;(AE, BE)&gt; for (A, B)<div class=\"where\">where\n    A: <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/default/trait.Default.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::default::Default\">Default</a> + <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Extend.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::iter::Extend\">Extend</a>&lt;AE&gt;,\n    B: <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/default/trait.Default.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::default::Default\">Default</a> + <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Extend.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::iter::Extend\">Extend</a>&lt;BE&gt;,</div></h3></section></summary><div class=\"docblock\"><p>This implementation turns an iterator of tuples into a tuple of types which implement\n<a href=\"aero_kernel/prelude/rust_2021/_core/default/trait.Default.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::default::Default\"><code>Default</code></a> and <a href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Extend.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::iter::Extend\"><code>Extend</code></a>.</p>\n<p>This is similar to <a href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.Iterator.html#method.unzip\" title=\"method aero_kernel::prelude::rust_2021::_core::iter::Iterator::unzip\"><code>Iterator::unzip</code></a>, but is also composable with other <a href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::iter::FromIterator\"><code>FromIterator</code></a>\nimplementations:</p>\n\n<div class=\"example-wrap\"><pre class=\"rust rust-example-rendered\"><code><span class=\"kw\">let </span>string = <span class=\"string\">\"1,2,123,4\"</span>;\n\n<span class=\"kw\">let </span>(numbers, lengths): (Vec&lt;<span class=\"kw\">_</span>&gt;, Vec&lt;<span class=\"kw\">_</span>&gt;) = string\n    .split(<span class=\"string\">','</span>)\n    .map(|s| s.parse().map(|n: u32| (n, s.len())))\n    .collect::&lt;<span class=\"prelude-ty\">Result</span>&lt;<span class=\"kw\">_</span>, <span class=\"kw\">_</span>&gt;&gt;()<span class=\"question-mark\">?</span>;\n\n<span class=\"macro\">assert_eq!</span>(numbers, [<span class=\"number\">1</span>, <span class=\"number\">2</span>, <span class=\"number\">123</span>, <span class=\"number\">4</span>]);\n<span class=\"macro\">assert_eq!</span>(lengths, [<span class=\"number\">1</span>, <span class=\"number\">1</span>, <span class=\"number\">3</span>, <span class=\"number\">1</span>]);</code></pre></div>\n</div><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.from_iter\" class=\"method trait-impl\"><a href=\"#method.from_iter\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.FromIterator.html#tymethod.from_iter\" class=\"fn\">from_iter</a>&lt;I&gt;(iter: I) -&gt; (A, B)<div class=\"where\">where\n    I: <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.IntoIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::_core::iter::IntoIterator\">IntoIterator</a>&lt;Item = (AE, BE)&gt;,</div></h4></section></summary><div class='docblock'>Creates a value from an iterator. <a href=\"aero_kernel/prelude/rust_2021/_core/iter/trait.FromIterator.html#tymethod.from_iter\">Read more</a></div></details></div></details>","FromIterator<(AE, BE)>","aero_kernel::arch::x86_64::tls::ProcFsCpuFeature","aero_kernel::fs::block::PageCacheKey","aero_kernel::fs::cache::INodeCacheKey","aero_kernel::fs::cache::DirCacheKey","aero_kernel::fs::MountKey"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()