// @pxl/client/origin/ingest.spec.ts
//
// GENERATED from spec/spec/asset/ingest.json
// Every shipped asset by its number below the asset root, file n at
// `ASSET_FILES[n - 1]` (ts/client/src/sys/fs/asset.ts): the bucket key
// (the SHA-256 of its bytes, which where it lies does not change), its size,
// and for a model its extent in ENU world units, measured from the glb. The
// bytes themselves are in the bucket.

export interface IngestedFile {
  key: string;
  bytes: number;
  /** ENU (x, y, z-up); absent for an image. */
  extent?: readonly [number, number, number];
}

export const ASSET_FILES: readonly IngestedFile[] = [
  {
    key: "asset/20f91809e69c66625190eb02aebef9ab543f5cf5cb17b8211b1002330584bfac.glb",
    bytes: 149728,
    extent: [1.25, 1.84, 1.5],
  },
  {
    key: "asset/9d3116da6c22eabf0b46efeeec7fceae8fbd01e101fddd4e6746379ff851e192.glb",
    bytes: 164356,
    extent: [1.33, 1.29, 2.02],
  },
  {
    key: "asset/94b654915d9de70071bcf0bfa13de379a3ba4fdfeb231f0fec49335232cbc821.glb",
    bytes: 131568,
    extent: [1.25, 1.33, 2.01],
  },
  {
    key: "asset/1e73aaf7d497dd7d2946badf92d189d2333f163e66dced91feb446bc9bdee850.glb",
    bytes: 165164,
    extent: [1.25, 1.81, 1.71],
  },
  {
    key: "asset/470d2481ba246ff957da0164707326e805c54dde553468a104e83ad9c7281723.glb",
    bytes: 130968,
    extent: [1.44, 1.33, 1.75],
  },
  {
    key: "asset/86dd5f030c9341486313b35b9e827500c430274a83b5d683cab5b1f55bc1024d.glb",
    bytes: 125424,
    extent: [2.2, 1.35, 1.59],
  },
  {
    key: "asset/2f6d0032e51c2d49b13b8e689d59eb39aa3ee6c32f2a1851213e11fc7593177d.png",
    bytes: 10915,
  },
  {
    key: "asset/57213ad1b06c1b6959c334215b4473039a7cd47c8500b65977df923e2e3e7929.glb",
    bytes: 128084,
    extent: [1.54, 1.46, 1.61],
  },
  {
    key: "asset/80fa5fd7c3652938d9e3c3024bb97dc9391a0e174fcc8b9459b63c613ed050f6.glb",
    bytes: 150768,
    extent: [2.34, 1.35, 1.43],
  },
  {
    key: "asset/b37162d91001eefda02bd111c8e836a0c4d805ab0191887459ded0d020624ec2.glb",
    bytes: 150400,
    extent: [1.54, 1.56, 1.99],
  },
  {
    key: "asset/c127d71ac8794f9710313bbb44800ae33a79f1182cf5c523c4233cc10a93851f.glb",
    bytes: 119544,
    extent: [1.26, 1.5, 1.58],
  },
  {
    key: "asset/0831346fb8ddb8e99e42e3f92fb48858551049dc7ea49e641d90fdfc65e057be.glb",
    bytes: 148892,
    extent: [1.88, 1.68, 1.43],
  },
  {
    key: "asset/b8432e91000ee6ad3f9223198709329ce46a5886c954ad851d11ae215b1fbbb2.glb",
    bytes: 91584,
    extent: [1.88, 1.26, 1.63],
  },
  {
    key: "asset/ac1ae804da301371509cff84206e3c6b78ff9c7d02411ef0cd004731090d6b82.glb",
    bytes: 141880,
    extent: [1.25, 2.31, 1.69],
  },
  {
    key: "asset/ab3562e1edb4dc6b14d5c7c9083ba3052a44ca0ccdcac2c15461d84397dc2f31.glb",
    bytes: 132676,
    extent: [1.56, 1.42, 1.71],
  },
  {
    key: "asset/978b137c139f65c5659632bd9c12a521fbd44b5756168653157b069db8f2faf9.glb",
    bytes: 141596,
    extent: [1.35, 1.5, 1.52],
  },
  {
    key: "asset/ba13a1975bd746c59e2dce8309736c26597c8a9839602f736e4ec8570ba84fa0.glb",
    bytes: 131008,
    extent: [1.94, 1.39, 1.46],
  },
  {
    key: "asset/64697c4a76357fe8d9688fe077a5e74e527a6602574fd0941895be84b5883703.glb",
    bytes: 172936,
    extent: [1.65, 1.9, 1.75],
  },
  {
    key: "asset/4f1ba41bd6380f0daa6205442f8fb797c167d7abceaaa58943eae0e6fc9044a6.glb",
    bytes: 174944,
    extent: [1.98, 1.78, 1.61],
  },
  {
    key: "asset/e001aea4fcd1c2dcb384774019f84183e6d0b638ebd986da65fee32bd93d1359.glb",
    bytes: 133624,
    extent: [1.34, 1.44, 1.5],
  },
  {
    key: "asset/a028fcccd8559652ea100fabaceb82ef53633e3474025eca593a753b008f52ab.glb",
    bytes: 154124,
    extent: [2.2, 1.9, 1.68],
  },
  {
    key: "asset/ff3d7ccae941ab06773a348b03446f97650043594ab1121a26514c3e2ea6cb29.glb",
    bytes: 130396,
    extent: [2.19, 1.35, 1.59],
  },
  {
    key: "asset/0e09d3c745a7daa1f53ecd320739b94d57a6ea1dfc498da01dd468f3f4c866a1.glb",
    bytes: 127032,
    extent: [1.25, 1.46, 1.58],
  },
  {
    key: "asset/8e94b6a8f44b38198f698f2262c3ed7aed34bd1cb3a016f7613fb5eea0897564.glb",
    bytes: 98964,
    extent: [1.25, 1.5, 1.5],
  },
  {
    key: "asset/cc70cdfd27ec6550b7a4524a3cc75fd594f94d384cac2bb68504b60d379f47d4.glb",
    bytes: 172348,
    extent: [1.35, 1.9, 1.71],
  },
  {
    key: "asset/0d4947d34ff32acf4a359c7f22ca784e057e7e72f622170a9a77b6fc88fdb70e.png",
    bytes: 8706,
  },
  {
    key: "asset/8cfcff43460da8b421f2a7fdfb43ec177321cad2746db9497cbf128d5806e2a8.glb",
    bytes: 273448,
    extent: [1.1, 0.5, 0.78],
  },
  {
    key: "asset/2288438e7baf9acc91a870c82dc00d66710bb486592cfc3474ef8ed93a03863a.glb",
    bytes: 252140,
    extent: [0.77, 0.42, 0.72],
  },
  {
    key: "asset/3cd9e1b5d6409fce0ba1af617d9e008c3069493185a4c100f49fb99a2626a055.glb",
    bytes: 252040,
    extent: [0.77, 0.5, 0.78],
  },
  {
    key: "asset/67f61708743bd34f91a4c2ba8160b61bf2e7d4ec2ca6647dfe99873422561410.glb",
    bytes: 258760,
    extent: [0.77, 0.5, 0.78],
  },
  {
    key: "asset/3ee3939f80d718945fcb9199f909a92c39669e38aeec7f544536b5dce6e79f98.glb",
    bytes: 248012,
    extent: [0.77, 0.53, 0.72],
  },
  {
    key: "asset/2ff4311897bfaf99be80d2fa13918db30f506607da0291640b2609d82b70e0ca.glb",
    bytes: 259488,
    extent: [0.77, 0.44, 0.67],
  },
  {
    key: "asset/77572792bfe2773b715b8cd8e18644b52b3e1f155fe10450254b50f9c364382a.glb",
    bytes: 246916,
    extent: [0.77, 0.34, 0.67],
  },
  {
    key: "asset/791fc0c203924c175c0a3d5b60d030daf36797b039506689cf7cd01ef5253b3c.glb",
    bytes: 247332,
    extent: [0.77, 0.39, 0.66],
  },
  {
    key: "asset/672a6506f7475bbd655da1d7ff712c1729a430f134c62385a4e5d3c1378acb40.glb",
    bytes: 263104,
    extent: [0.77, 0.46, 0.79],
  },
  {
    key: "asset/dd12b2e75ffb1cdb45aaec3916c3d6e732929ccb4a8b89b946f69baaabfa64da.glb",
    bytes: 245184,
    extent: [0.77, 0.34, 0.72],
  },
  {
    key: "asset/cd76681090ce29861b055b0f8b2bdcf84bfccc4ca816f428df42a9bb76d6e1b6.glb",
    bytes: 242704,
    extent: [0.77, 0.34, 0.68],
  },
  {
    key: "asset/ed151fc47c5cd6be9693c524e92f045170c965819f4f81b28ed66dcc3c1085bf.glb",
    bytes: 249388,
    extent: [0.77, 0.34, 0.67],
  },
  {
    key: "asset/8588d28543564e2214138851a8aa240e255dd87a193270188b6d9489fefcae07.png",
    bytes: 80903,
  },
  {
    key: "asset/1c4c8d35d392032f3ef6806bd8dafca4ab53b69b5567ed52a42443c4d4e70b96.png",
    bytes: 91859,
  },
  {
    key: "asset/93bc8ca2327278d4dedf3877163b988c96a7fe9e37717fdaf0cec35a904ce0fa.png",
    bytes: 91351,
  },
  {
    key: "asset/d66816120e900a38bb468aac934c586d4d5b3a7aebda492c8626a6dd452848a1.png",
    bytes: 85804,
  },
  {
    key: "asset/7c2391210d4018f43a7695ce30fbb5b602e2b9106ab0c5f468dcf33d8ce5e94f.png",
    bytes: 91753,
  },
  {
    key: "asset/55c1809e6cf955ff4f22c26c1b724273e035c1aff6884608e05aae531427ab32.png",
    bytes: 77560,
  },
  {
    key: "asset/1d0cb4d20a4f3b80a4be789fc33ae8f71b2691f21703494118c09a2862bea2b0.png",
    bytes: 89240,
  },
  {
    key: "asset/26ada63041722a22b838f5537d2c1eb27f4e267e75917c6ac36a43fd36ffc836.png",
    bytes: 85778,
  },
  {
    key: "asset/7a90030363341d053e9d3073f8294f6db232e000ae10550f062500bf3d326589.png",
    bytes: 89635,
  },
  {
    key: "asset/ab18e5a48827773262c0629dafac6c51422c230a2c66ffd5931d2b83165c7efc.png",
    bytes: 85799,
  },
  {
    key: "asset/682694624b5ec9c80b6d41412f9b16748ac16da8d5efc4dce5d041e7c28bb9ee.png",
    bytes: 95817,
  },
  {
    key: "asset/0c40388632f043b5e47e24e03e84e11a37347057f71ad275d4966e9b875cd5a7.png",
    bytes: 84310,
  },
  {
    key: "asset/b08576ec1a70440b8b0b59d3e01a9c1e6f12518a4a610d4cb69cac061421a874.png",
    bytes: 91559,
  },
  {
    key: "asset/6da2cf5f8dd2c146ff9addd9d45416087bd1e8ab5ccf75d65c37c06bdbe6d039.png",
    bytes: 78320,
  },
  {
    key: "asset/383731be87d80f67be4f25b8bc40ca921c034e359e4e9e15cc42dfc34b3f524d.png",
    bytes: 88054,
  },
  {
    key: "asset/6394424253433a73ae94ae5c4de37eb56bd1e831d519cd1bcdb2d3eac5bbc824.webp",
    bytes: 882258,
  },
  {
    key: "asset/b5713e6cdf0d84a951154d3b8ab1f529cf90cfdb8997c8156e1ec3104e784367.png",
    bytes: 91907,
  },
  {
    key: "asset/59530abe14367ed90a921c0c970145037b75bfe97222780fb86b48b4f56a3ae9.png",
    bytes: 102120,
  },
  {
    key: "asset/da27b6c52b6c9ab7ea0de9a73350d74846a76da3d850bc28e11d08f7e616c886.png",
    bytes: 104341,
  },
  {
    key: "asset/8338f7d8524446cb38404a04856a079392fd46e95b9177f1d61b067251d96524.png",
    bytes: 95619,
  },
  {
    key: "asset/65c897a61f7aaf91dc568f85859791c487ad73514f0cf33da80f1f6893e5c655.png",
    bytes: 99071,
  },
  {
    key: "asset/e611f29f041b1f5b12ec18267feb5161061453ae3b14643d2844460d95b7d69c.png",
    bytes: 111942,
  },
  {
    key: "asset/8d84f215355691e13343a86550ba29ba8df2f68acae59a2adc0f259841402ec1.png",
    bytes: 99695,
  },
  {
    key: "asset/e13cb3b852c6cb34c1c116815e4d53d6d1345cd9bdd029a9cdf60e1b8ddb23db.png",
    bytes: 99505,
  },
  {
    key: "asset/ff0c589fe24035da5bc988b14124fd0aab0cbc815ad0dfdecfbd39c03ad1ea0a.png",
    bytes: 90625,
  },
  {
    key: "asset/6c061df100babc0f61a6826bc6a7bf958029f4970c132a1689a70edfa12c672b.png",
    bytes: 97093,
  },
  {
    key: "asset/6140fafdb61d27829a3f7d6b84d9721f23e403d9ee6917fca94ba23685db28d2.png",
    bytes: 151936,
  },
  {
    key: "asset/4f0181eee525ee574e18abf7cd2658128cd0e4084cb090e14845a2f1f2e80543.png",
    bytes: 89147,
  },
  {
    key: "asset/abedd05652ca4e91aa6158a560b6e8a036c466ca3e6914a509020c36cdf0a44b.png",
    bytes: 128461,
  },
  {
    key: "asset/26d6b02c311ad809058bc24ab9c6b95ef24768b325d1b2a4e5f894bd6cebdc4f.png",
    bytes: 117312,
  },
  {
    key: "asset/2331d16774f646cda52d96c0feb1cc4a36868197e3fc41de3169cb0237f2d5b3.png",
    bytes: 134398,
  },
  {
    key: "asset/02939fa052bbbedc874c127c1247bf5434ee68a5239744cad031bf1299022adf.png",
    bytes: 140030,
  },
  {
    key: "asset/7d7cf7954651b841ea29decb27d56f32dfd68e0df319756bc2c577d151102592.png",
    bytes: 104855,
  },
  {
    key: "asset/5a66f49040fe373546aa905c47f52d61901ed516477f70a8510ceadee118cb16.png",
    bytes: 127593,
  },
  {
    key: "asset/a710a0e4e1ef5deb751e29f842d18408ab9916a2df44a7df1ef47cf5f8c197e8.png",
    bytes: 86914,
  },
  {
    key: "asset/7e771f1a114b170db5751fafcc4a0ef3fa9cfb1a6f66ca7fd7e26d631d134c5c.png",
    bytes: 109088,
  },
  {
    key: "asset/141a2d1951e7558cac04a9f344e249156c11bdf025fdc1672d303a848bb622a4.png",
    bytes: 112470,
  },
  {
    key: "asset/e3493d37e6092ddf94b3576e247082b731c22689a2dbb4dabfae1d30bba373ac.png",
    bytes: 111522,
  },
  {
    key: "asset/9d8ca8b33a7ffc7084d3d0800fc365168842908c2c6abb045ac23371a1dc144c.png",
    bytes: 114639,
  },
  {
    key: "asset/53e1760c470409f6601af35298252e348528523ea0853fb77836d3ebdea59f2b.png",
    bytes: 148707,
  },
  {
    key: "asset/2fa2fc211a789e61643e54f75a85b9ca835637a3287366068f4be62a50eca7d1.png",
    bytes: 102974,
  },
  {
    key: "asset/0d2e889ed897592246c67f4a9437c1fb421702922cd04b6341fdec277500c224.png",
    bytes: 121116,
  },
  {
    key: "asset/78430fec9e930090fe92944230dc0dbc106e6dd4e8237e6ac24fb10ab1c1f0fa.png",
    bytes: 128217,
  },
  {
    key: "asset/7ea8272c68816107b3a08c53ec2304a9b20d6f39c0e184b787a87458cbbe310b.png",
    bytes: 102375,
  },
  {
    key: "asset/31cfe5b7e39087830b5efaac61d6b76c8f1dbfaacaafd1f4480f0d2e5a1fa614.png",
    bytes: 138501,
  },
  {
    key: "asset/59d7c810da596f8f8b32244655a165925673f3496a9f3911e2ded4368529004d.png",
    bytes: 117040,
  },
  {
    key: "asset/2cc7941b6ad86e3662a456aaa3b2f9c1f2996d2c37c81ae0b2e3f1f2c4725018.png",
    bytes: 94188,
  },
  {
    key: "asset/a08b94a5773f939c61d73f08fce04af60a6bccab0dc0bb89c9ab0910de0a1efc.png",
    bytes: 106265,
  },
  {
    key: "asset/4570fe9422e9feacb39bf3d4b1670c9089e451ce94a488c1c0cf793533f5fb50.png",
    bytes: 112097,
  },
  {
    key: "asset/3520afe96ddf77ce50c505125dd1e267e108b7eb58d4f6f1221c715452d3be0c.png",
    bytes: 133207,
  },
  {
    key: "asset/af069ef26872d5de3dee5bc79db72e7bbbf71b90de3e0f73b89f0d4f42af933d.png",
    bytes: 122249,
  },
  {
    key: "asset/30b8b36eedff694bab60a8c1130a585785425323a022ebe4ce3e86a1c91c3836.png",
    bytes: 119970,
  },
  {
    key: "asset/4ad6b210429deeb85709cc31fa7ff25ccd3f75e1a82306ef235742bab352b768.png",
    bytes: 113363,
  },
  {
    key: "asset/2afc170faec14fb93e8ae0db3b7699ddee6afa2bd815bb7c582096bbf8cd676a.png",
    bytes: 81119,
  },
  {
    key: "asset/1d6b9d4fd48a176cde80054c042a07dd9aca697fb2bfe7c6e6062e232e954caf.png",
    bytes: 135463,
  },
  {
    key: "asset/48b31c4cd0f99278510db1f0ce1a6acf72018c74df884e917500fac2c76627ab.png",
    bytes: 86990,
  },
  {
    key: "asset/55653ea52f1d869fa80043b47ad92b81fc401d17f6b3e7b6f1cf7b497ffcf743.png",
    bytes: 96984,
  },
  {
    key: "asset/4c40800f0d1f37f16c44598da934c7eb84955df1ac7ae24eb2d27890eca3457d.png",
    bytes: 78323,
  },
  {
    key: "asset/30962727cac9863189ecec9626f81e5d11e452bd8c180b7754c602a2e1b050b1.png",
    bytes: 98627,
  },
  {
    key: "asset/b1cb49a47cd1331ddd47f9444ea5b8313702ab46cb1ab8d2498bbf97f0922d98.png",
    bytes: 93799,
  },
  {
    key: "asset/65f3a1dae26b7691709415c3019f6d8abc4d8d84358653cd85b85f72a63001cf.png",
    bytes: 91746,
  },
  {
    key: "asset/d3c212298b657d694fa0a66fc8c08c139d96a415051f189eb972c2e3d627f27d.png",
    bytes: 96005,
  },
  {
    key: "asset/19664ab80d717f8e0a3386d697c0b3c3201dfe3071cc10960a84df6d05c30ea5.png",
    bytes: 108528,
  },
  {
    key: "asset/047f51ab8a080f5ab9edcb72b3f9481d5485584f9e5e10707696833c3179c2a4.png",
    bytes: 98268,
  },
  {
    key: "asset/1f2cb5030ec79c71ff5bbdd15b465bc7fcff9cce17839f2c09f0df876a5a69d1.png",
    bytes: 90540,
  },
  {
    key: "asset/818786f84e9acd9e414c0e9da5be9729bf482bad0c81a41d376008a2a44c6a70.png",
    bytes: 93125,
  },
  {
    key: "asset/cba4a0215c84215c18645002d9a3650eef91072ed386e8a1e846df569b4edcc4.png",
    bytes: 95707,
  },
  {
    key: "asset/4949c9539bcd850ba34b348dd0a4034b88a6056566ab594b4bded42eeb123c2b.png",
    bytes: 133232,
  },
  {
    key: "asset/391d9f05d108aea37a19bc86dfa2e0e2a4d026a8afc2052413738a2a50ab3c24.png",
    bytes: 102877,
  },
  {
    key: "asset/ead3dcd205bce4439c701f966015f893b3b133904a0dbbeb96b1bd55bf78d734.png",
    bytes: 98485,
  },
  {
    key: "asset/8566a6c1e83e9acf72e3d32b6e2751acd93d54245b83a13ec0c4d9cc5f9cf723.png",
    bytes: 90535,
  },
  {
    key: "asset/8566a6c1e83e9acf72e3d32b6e2751acd93d54245b83a13ec0c4d9cc5f9cf723.png",
    bytes: 90535,
  },
  {
    key: "asset/2375509e9210f0cd4b8257f47327a97b253dd811c7007a3d5aafb82f8d7fe288.png",
    bytes: 129476,
  },
  {
    key: "asset/56a1ca20ddc06e8e53d5dd7bc09f6d3fb84f72ef842da9a33fb9d522901fdd14.png",
    bytes: 109680,
  },
  {
    key: "asset/02e7910434d7db0251bcda158b9a5add761b931151b5d56ae93de9ca6d0ae833.png",
    bytes: 126398,
  },
  {
    key: "asset/7fe0fa419a71475c9e4b56e414b5e88da32553c0be9b02811496f4fc012cba73.png",
    bytes: 137824,
  },
  {
    key: "asset/92943c88f04cc9a220712fec2ca52cda1668124928cdac47e641eb20008960f9.png",
    bytes: 100685,
  },
  {
    key: "asset/c9e8bfa18db2a40a50c477994fba51460899d02d514f04daa378421c4588f46e.png",
    bytes: 97770,
  },
  {
    key: "asset/1d367d4edd7ac24b937854e39044361329bbf4abfd44c37df150a2b4cd2e53b7.png",
    bytes: 86712,
  },
  {
    key: "asset/dcd20492982c9cafadc236ed953d57b863628314eab16352710a5a5482464102.png",
    bytes: 99091,
  },
  {
    key: "asset/ead3dcd205bce4439c701f966015f893b3b133904a0dbbeb96b1bd55bf78d734.png",
    bytes: 98485,
  },
  {
    key: "asset/d4f35ad1275ede96a73648dec48b6de39ab5ddb20fe227d52348959767135df4.png",
    bytes: 84649,
  },
  {
    key: "asset/62a6bb48869e38d4fdd40a0d55784585944d1c3c0ff2bf09944954e4717b047c.png",
    bytes: 127077,
  },
  {
    key: "asset/fc53c0ccca771fcc07c1771c5f28909a002ac77e99e4e3aa40a9a4de52e5c13e.png",
    bytes: 120418,
  },
  {
    key: "asset/7eb528c28c14297be1729d163a29b79997b0969783626a6a2c5529ceb74e2a35.png",
    bytes: 115253,
  },
  {
    key: "asset/f2c64b0d0d7ae6682dafb88eebbe82948c08776e2dbdd814a6079f87b7dec927.png",
    bytes: 109668,
  },
  {
    key: "asset/74a6f49a3374a3348d754952a98c16cf7a4155e173e72f7e3040e836c0f40fe5.png",
    bytes: 125591,
  },
  {
    key: "asset/5f9d444069a665b70750e3addecf6f34fad5f24476f6cf68ed6b2318edc87e1a.png",
    bytes: 123681,
  },
  {
    key: "asset/acc9699dd532f80d090101df81a52a93595e25662806e19020cc936a38b0bf74.png",
    bytes: 126887,
  },
  {
    key: "asset/95cb7ebeff94dd30bcd46c521dfac70d3cc1b4ff03d48256a8eb2d24d65d003d.png",
    bytes: 103487,
  },
  {
    key: "asset/fe22c2f40f8a707ccfef4c699cbd023c34b2a3463f0196731d8f4bed0497ee6d.png",
    bytes: 79619,
  },
  {
    key: "asset/f3622a03a20c6696065cae9cbe391351be873508af190c2ebd1d420c055787a5.png",
    bytes: 12371,
  },
  {
    key: "asset/bd5c9d4c3b4bdd254a66b8426563a68b1487ae7a5076df3a2488e6ffed7be64f.glb",
    bytes: 197804,
    extent: [1.3, 2.85, 1.1],
  },
  {
    key: "asset/b532ea7d2c59f7f6b22b138cf1955218a2c1898f1cea932af4d3fd563c3959b7.glb",
    bytes: 172216,
    extent: [1.5, 2.55, 1.3],
  },
  {
    key: "asset/9df31f6e89e927a8176fc792aa125f0b19dc1ba29d2025bfef7c4f4d6ab1778a.glb",
    bytes: 207572,
    extent: [1.5, 2.7, 1.3],
  },
  {
    key: "asset/3803539718ffd3b84b515dbce8ed6b489f1ff5be58edb1903d2c5db5c584bdc7.glb",
    bytes: 175608,
    extent: [1.5, 2.75, 1.5],
  },
  {
    key: "asset/ed4ed56e8e5ed98db050af09ed2062b3b6bb7b93eab1e563cb6774b387b233a1.glb",
    bytes: 175664,
    extent: [1.5, 2.75, 1.35],
  },
  {
    key: "asset/4aac939dc33195e35caf8c382ee3cb170054da763577f22d3c22692ec6afccdf.png",
    bytes: 11143,
  },
  {
    key: "asset/da4349701dd3fc474f7bb03cfe8b230bec0919025d8d1ecac0d1061a23c5e910.glb",
    bytes: 19252,
    extent: [1.02, 1.02, 2.41],
  },
  {
    key: "asset/a9af8768a5328c62694a42f67858a88eea974ad4540c5c3c54a7a70fdead7505.glb",
    bytes: 21744,
    extent: [1.02, 1.02, 2.97],
  },
  {
    key: "asset/7f372e338cbf5514a41e8d2afda81acd67277a4c5ad4e6e35a9acfa4a6e3bc4b.glb",
    bytes: 13176,
    extent: [1.02, 1.02, 2.75],
  },
  {
    key: "asset/2dbb718a81d9c3697c069495f58840f6c2002c8cdd13388be560c2199f6d03e7.glb",
    bytes: 20700,
    extent: [1.02, 1.02, 2.97],
  },
  {
    key: "asset/e8d81b97297de62699b92559cee7b247da84b1815df2515e42fd5a318a30b8d7.glb",
    bytes: 18188,
    extent: [1.02, 1.02, 2.41],
  },
  {
    key: "asset/a94b4dc4f77a4b8aec96bde94f63912a1ce12aa98eec2c1e9a37c89e20a5f091.png",
    bytes: 10965,
  },
  {
    key: "asset/3080ca745df311d59cbe445a31da6cadfd7aa5f14e4846015f310746a6c23fe1.glb",
    bytes: 35944,
    extent: [1.18, 1.33, 2.11],
  },
  {
    key: "asset/299d2ceb57f8ca20048b955e4e2aa509ea16b1403dd2f178db9396dec5221760.glb",
    bytes: 35960,
    extent: [1.18, 1.33, 2.11],
  },
  {
    key: "asset/1370654dc1da3615f51e31880e71d78990d5dbe7a4a0480abdbfc840fb89a6ad.glb",
    bytes: 35936,
    extent: [1.18, 1.33, 2.3],
  },
  {
    key: "asset/0bd5e72e2aa430e74625b3a11bd809c669c7cc58a300b028ff06814d45c2d213.glb",
    bytes: 35920,
    extent: [1.18, 1.33, 2.3],
  },
  {
    key: "asset/e264660f3fc9ecbd642bdb7fb7b7dc10755a061560a66651baf48274c729af09.png",
    bytes: 12684,
  },
  {
    key: "asset/20c6c7c23c005e8cda1c1e922473c0fc9c93fc7c1d1bc5a637d640424f811cc3.glb",
    bytes: 156432,
    extent: [1.2, 2.4, 1.6],
  },
  {
    key: "asset/9f63a93e65e530982bf5ac3ad15b3c9f3b2608561b3773164d970c67bd2272bd.glb",
    bytes: 104168,
    extent: [1.1, 2.4, 1.68],
  },
  {
    key: "asset/8dfe9d20667c1534652286149fb0e1639ff48975a4187d3ad2fd956add0bba2a.glb",
    bytes: 154620,
    extent: [1.43, 2.6, 1.66],
  },
  {
    key: "asset/ed82b635fa1dedd8017e40442a88cc8e2f05362f696a7c8b6e516db9037f8450.glb",
    bytes: 142648,
    extent: [1.32, 2.6, 1.66],
  },
  {
    key: "asset/f6a03c86e7147764de6aac135433a428ee503d42852ecd101c1795a278480742.glb",
    bytes: 22948,
    extent: [1.78, 3.87, 2.6],
  },
  {
    key: "asset/82e85f716f059b3e5cd90088a6e5b7c7722518ff93e1592afe3fff076b617830.glb",
    bytes: 17636,
    extent: [2.75, 2.37, 0.85],
  },
  {
    key: "asset/35c8e2adae1d3f0fe9d0e37c65422f1c6c6f1f10d818f9bde273a9da264eef4c.glb",
    bytes: 35276,
    extent: [1.78, 3.77, 4.74],
  },
  {
    key: "asset/d6d5605408e8be888551588b4c73d2953414250e2f29864ee133e9ba5ee2e8b2.glb",
    bytes: 16520,
    extent: [1.78, 3.37, 1.5],
  },
  {
    key: "asset/311138350e74e35c5497a1bd8076a01740b7c302bed11027639fa553459a68de.png",
    bytes: 8814,
  },
  {
    key: "asset/6e6d529a81c239f936f933f38ee3b529a418a3247deeaa122b9b313a5a8bc21d.glb",
    bytes: 27608,
    extent: [2.97, 1.85, 1.63],
  },
  {
    key: "asset/08a8d12f1a55ac389db47fff76ec0a36fd66d66df71662d683335b1b2d3089ab.glb",
    bytes: 27608,
    extent: [4.26, 1.75, 2.07],
  },
  {
    key: "asset/1c6b7407824dffb3f0512eacb16b03eeb5f8540bf4919793299fdf2f4b61405f.glb",
    bytes: 27608,
    extent: [8.1, 1.85, 1.31],
  },
  {
    key: "asset/adbbf0299636e18a416d712b4291c69a8464d57cf96b97d95d6d14c939bf9486.png",
    bytes: 72,
  },
  {
    key: "asset/e6977d49922c1676da63b5d6179388ba61f1ad7a9ae0b40408718b7cc7123881.png",
    bytes: 165698,
  },
];

/** The texture pack as one image, the file numbered `image`: `columns` tiles
 * of `cell` pixels per row, tile n at column n % columns and row n / columns,
 * `tiles` the number of the file each holds, and `top` the tile each cube
 * wears on top, by the cube's number. */
export const PACK = {
  image: 54,
  cell: 256,
  columns: 8,
  tiles: [
    39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 55, 56, 57, 58,
    59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77,
    78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96,
    97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112,
    113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127,
    128, 129,
  ],
  top: {
    260: 0,
    268: 1,
    263: 2,
    267: 3,
    266: 4,
    259: 5,
    265: 6,
    258: 7,
    270: 8,
    262: 9,
    271: 10,
    261: 11,
    269: 12,
    257: 13,
    264: 14,
    255: 18,
    64: 19,
    65: 23,
    13: 25,
    30: 26,
    2: 27,
    4: 28,
    16: 29,
    17: 30,
    21: 31,
    14: 32,
    15: 33,
    11: 34,
    19: 35,
    31: 36,
    7: 37,
    5: 38,
    22: 39,
    1: 40,
    24: 41,
    9: 42,
    12: 43,
    8: 44,
    29: 45,
    10: 46,
    27: 47,
    23: 48,
    25: 49,
    66: 49,
    18: 50,
    20: 51,
    3: 52,
    6: 53,
    26: 54,
    28: 55,
    34: 66,
    35: 67,
    36: 68,
    33: 69,
    55: 70,
    39: 71,
    38: 72,
    37: 73,
    51: 74,
    42: 75,
    53: 76,
    48: 77,
    41: 78,
    49: 79,
    46: 80,
    54: 81,
    43: 82,
    44: 83,
    45: 84,
    52: 85,
    40: 86,
    50: 87,
    47: 88,
    32: 89,
  } as Readonly<Record<number, number>>,
} as const;
