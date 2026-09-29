# AetherCore — خطة التنفيذ المعماري P77 وما بعدها

## §0 Verdict — أعلى عشرة تغييرات قيمةً، بالترتيب

1. **INFERRED — P77:** إغلاق منافذ عرض النصوص الخام في العربية، مع اختبار حقن نصوص إنجليزية ومعرّفات مجهولة؛ تساوي مفاتيح القاموس وحده لا يثبت تكافؤ المنتج.
2. **INFERRED — P78:** جعل تقييم الإصلاح عملية محدودة العمر ذات حالة فشل واضحة وتقدّم صادق وإلغاء فعلي؛ الانتظار الأبدي يهدم الثقة أكثر من نتيجة «غير متاح».
3. **INFERRED — P79:** ربط Care بالتشخيص والتحضير الآمن وإظهار سبب كل استبعاد وتسجيل التشغيل في الخط الزمني؛ لا تعني قائمة خطط فارغة أن الجهاز سليم.
4. **INFERRED — P80:** عرض نطاق القياس وعمره ومصدره وحدوده بجانب كل حكم، وفصل عدم التغطية عن الصحة؛ هذا أساس التشخيص الاحترافي.
5. **INFERRED — P81:** استكمال صحة التخزين وSMART/NVMe مع فكّ آمن للبيانات وهوية قرص مستقرة؛ المخاطر على البيانات أعلى قيمةً من أرقام استهلاك إضافية.
6. **INFERRED — P82–P83:** قياس الحرارة والطاقة وWHEA والإقلاع والشبكات وصحة Windows Update دون افتراض أسباب لا تثبتها القياسات.
7. **INFERRED — P84:** استكمال دورة تعريفات الأجهزة: فحص محلي صادق، بحث رسمي بإذن، تثبيت محمي، ثم تحقق واسترداد؛ لا يُستنتج التقادم من التاريخ.
8. **INFERRED — P85:** إكمال SFC/DISM والتنظيف المحدود بالفئات مع تحقق بعد التنفيذ؛ النجاح هو تحقق الأثر، وليس انتهاء العملية برمز صفر فقط.
9. **INFERRED — P86:** مساعد ظاهر من كل شاشة، يتحدث من أدلة فعلية وبالعربية، ويصل إلى نتيجة مقبولة أو رفض واضح ضمن ميزانية زمنية معلنة.
10. **INFERRED — P87:** بوابات تثبيت حقيقية EN/AR، واختبارات سلوك وعزل للأعمال الثقيلة، وقياس زمن CI دون إسقاط ختم المصدر أو تجميد الاعتماديات.

### نطاق هذه الوثيقة ومعيار الدليل

- **MEASURED:** أُجري أولًا `git fetch origin && git checkout --detach origin/main` داخل `/Users/hasanalaaa/dev/aethercore-review`؛ لقطة المراجعة `1581d22`. كانت `AUDIT/` غير متتبعة وبها `P75-REVIEW-2.md`؛ لا تُحذف ولا تُضم تلقائيًا.
- **MEASURED** هنا تعني قراءة مصدر أولي يثبت وصف الكود أو العقد، **ولا تعني اختبار Windows**. **INFERRED** تعني اقتراحًا/فرضية يجب أن يثبتها المنفّذ. نتائج تقارير المراحل تُنسب إلى التقرير، ولا يعاد وصفها كاختبارات هذه الجلسة.
- كل هدف وخطوات ومعايير أداء مستقبلية أدناه **INFERRED** ما لم تُوسم بخلاف ذلك؛ كل مهمة تذكر أساسها المقروء منفصلًا. لا ادعاء بأن اختبارات red-before نُفذت في هذه المراجعة.
- لم يُبنَ أو يُختبر أو يُشغّل المنتج هنا. لا تعديلات سوى هذا الملف، ولا commits أو branches أو pushes. لم تُقرأ ملفات مولّدة أو lockfiles أو MANIFEST أو release/out/target/node_modules أو الأرشيفات أو مستندات مراحل قديمة.
- مصادر الطبقة الأولى: `phase21-workspace/docs/LEDGER.md` §1، `phase21-workspace/docs/phase75/P75-REPORT.md`، `phase21-workspace/docs/phase75/AMBITION.md`، `DESIGN.md`. تليها العقود الحية في `phase21-workspace/crates/contracts/proto/` ثم المصادر المحددة لكل مهمة.
- **MEASURED:** `assistant.proto:AskAssistantRequest` و`insights.proto:RequestInsightRequest` يحملان `locale` بالفعل في P76. لا تعاد إضافة اللغة إلى العقد. `care.proto:GrantCareSessionConsentRequest` يحمل digest؛ التقرير P75 §2 يوثق الموافقة أحادية الاستخدام وربطها بالمالك. لا تُستبدل هذه الحماية بموافقة عامة.
- **INFERRED — قاعدة الدمج:** هناك وكيل يدمج الآن؛ على منفّذ كل جلسة مقارنة الملفات المذكورة مع أحدث `main` عند بدء العمل. إذا تحقق معيار المهمة بالفعل، أضف/ثبت اختبارًا ناقصًا فقط أو سجّل أنها أُغلقت. لا تعاود تنفيذ صف CLOSED من P75/P76 اعتمادًا على تقرير أقدم. أرقام P77+ هنا مقترحة، وتُحجز لدى المنسق قبل التنفيذ.
- **INFERRED — تفسير التصميم:** تُحفظ ألوان `DESIGN.md` الستة وتمييز الرفض بالشكل والنص. عبارات مثل Repair Registry وinstant cause وApply AI Strategy ليست تصريحًا لبناء منظف سجل أو تشخيص سببي غير مثبت أو تنفيذ ذاتي من النموذج.
- استُدعيت تعليمات Second Brain، لكن تشغيل heal/retrieval مُستبعد لهذه المراجعة لأنه يخالف قيد عدم التشغيل/الكتابة خارج الملف. لا تعتمد الخطة على ذاكرة غير متحققة.

### قيود التنفيذ المشتركة

**INFERRED — تنطبق على كل مهمة:**

- **H1** لا اتصال شبكي في الراحة، ولا telemetry خارج الجهاز؛ لا تحميل رموز تصحيح أو نموذج أو تعريف ضمن فحص محلي.
- **H2** كل تغيير مدمر خلف موافقة المالك المرتبطة بالخطة والهوية والمحتوى؛ حفظ الحاجز الحالي وmachine-wide lease.
- **H3** كل insight يستشهد بدليل أو يُسقط؛ الغياب وعدم الصلاحية وعدم حداثة الدليل ليست قيمًا سليمة.
- **H4** EN/AR متكافئتان؛ أسماء الأجهزة والملفات والمصطلحات التقنية بيانات معزولة الاتجاه، لا تصبح حيلة لعرض نثر إنجليزي مملوك للمنتج.
- **H5** لا توسيع DACL للـpipe أو تخفيف service hardening؛ جمع LocalSystem لا يمنح المستخدم قراءة أدلة مستخدم آخر.
- **H6** ختم المصدر وتجميد الاعتماديات يبقيان بوابات إلزامية. لا مكتبة جديدة مطلوبة افتراضيًا؛ أي ميزة dependency أو SDK/FFI غير متاح في الاعتماديات الحالية: **OWNER DECISION** قبل التغيير وإعادة إصدار freeze وفق مسار المشروع.
- **OWNER DECISION — D0:** كل تغيير wire، حتى إضافة اختيارية، يحتاج قبول المالك قبل تنفيذه. التوصية: إضافات ضيقة، tags جديدة بعد فحص أحدث العقد، لا إعادة ترقيم ولا استغلال technical strings لحمل JSON غير موثق. تجديد bindings عبر آلية المستودع، لا تحرير مولدات يدوية.
- **INFERRED — حجم الجلسة:** المهمة الواحدة أدناه جلسة مستقلة، عادة 100–300 سطر منطقي مع اختبار محدد؛ سقف 400 سطر غير مولّد. إذا تجاوز الحل هذا، يُسلَّم الجزء الآمن المحدد وتُفصل متابعة قبل التنفيذ، لا تُضغط ميزة كاملة في جلسة. لا تنفيذ مرحلة كاملة في جلسة واحدة.
- **INFERRED — قبول كل جلسة:** اختبار سلوكي صغير يفشل على الأساس بالسبب المحدد، ويمر بعد التغيير؛ existing green test ليس red-before جديدًا. اختبارات fixtures تثبت التحويل والمنطق فقط، وWindows acceptance يثبت API/التثبيت. تعذّر الجهاز يترك النتيجة INFERRED، لا green. شغّل بوابات المشروع المطلوبة في التنفيذ فقط، وختم المصدر من تغييرات الجلسة المعروفة، دون تضمين عمل الوكيل الآخر.

## §1 P77 — حدّ عرض يمنع تسرب اللغة والمعرّفات

**MEASURED — الأساس:** `apps/ui/src/lib/i18n/semantic.ts:29–43,228–304` يعيد قيمًا مجهولة كما هي، وبعض regex يترجم الغلاف ثم يحقن `detail` الإنجليزي. `LocalizedOwnedText.svelte:18–22` يعرض النص غير المترجم داخل `TechnicalText`. `DeepScanPage.svelte:159` يعرض `collector.id` عنوانًا. `tests/wire-values.test.ts` يختبر قيمًا معروفة، ولا يغطي قيمًا مجهولة أو نصًا إنجليزيًا داخل argument. المسارات في المراحل التالية مختصرة نسبةً إلى **`phase21-workspace/`**؛ `.github/...` و`AUDIT/...` و`DESIGN.md` من جذر المستودع. كل مسار تعديل قائم أدناه جرى تحديده بـ`rg --files` أو قراءته؛ ملفات **NEW** مقترحة عمدًا ويُذكر مجلدها القائم.

### P77-01 — فشل مغلق للنص المملوك والمعرّف المجهول [INFERRED، 200–300 سطر]

- **الهدف:** لا يصل unknown enum/id أو backend English prose إلى الملخص المرئي أو الاسم الميسّر بالعربية.
- **الملفات — MODIFY:** `apps/ui/src/lib/i18n/semantic.ts`، `apps/ui/src/lib/i18n/catalog.en.ts`، `apps/ui/src/lib/i18n/catalog.ar.ts`، `apps/ui/src/design/primitives/LocalizedOwnedText.svelte`، `apps/ui/tests/wire-values.test.ts`.
- **النهج/المرجع:** إعادة استخدام `hasMessageKey` و`t/td` و`Intl` الحالي؛ [Svelte compiler/AST](https://svelte.dev/docs/svelte/svelte-compiler)، و[Unicode bidi isolation](https://www.w3.org/International/articles/inline-bidi-markup/). لا dependency جديدة ولا ترجمة آلية وقت العرض.
- **الخطوات:** (1) أضف اختبارات للقيم المجهولة و`Hardware telemetry: Backend exploded` و`preflight rejected: Unknown message` قبل التغيير. (2) استبدل raw fallback للـsemantic labels برسالة Unknown/Unavailable من القاموس في اللغتين؛ لا تُغيّر الهوية المستخدمة للمنطق. (3) اجعل `localizeOwnedText` يعيد fallback محليًا للنثر غير المعروف، ولا يضعه في `TechnicalText`. (4) راجع فقط regex داخل الدالة: placeholders رقم/نسخة/اسم جهاز مسموحة حسب نوعها؛ أي `detail` نثري يعاد توطينه أو يستبدل برسالة محلية. لا whitelist مبنيًا على وجود حرف عربي. (5) احتفظ بالخطأ الخام في السجل المحلي الحالي، لا تُرسل telemetry ولا تُنشئ مخزنًا جديدًا.
- **قبول red→green:** unknown state لا يساوي الإدخال في EN/AR؛ النص الإنجليزي المحقون لا يظهر حتى داخل جملة عربية؛ known messages تبقى صحيحة؛ أرقام/مسارات/أسماء أجهزة مثل `Samsung 980` لا تتلف؛ rendering fixture للنص المجهول لا يحتويه في DOM أو `aria-label`/title. اختبارات النص الحالية تبقى خضراء.
- **المخاطر:** إخفاء سبب مفيد وراء fallback؛ يُعالج بالرمز التقني الآمن/معرّف الارتباط عند وجوده وبالسجل المحلي، لا بعرض جملة خام. لا توسع إعادة التصميم إلى جميع الصفحات في هذه الجلسة.
- **القيود:** H3،H4،H5،H6. لا wire change ولا OWNER DECISION لازمة لهذا الحاجز.

### P77-02 — أسماء جامعات البيانات وحالاتها بدل IDs [INFERRED، 150–250 سطر]

- **الملفات — MODIFY:** `apps/ui/src/features/intelligence/DeepScanPage.svelte`، `apps/ui/src/features/diagnostics/ProviderFaultsPanel.svelte`، `apps/ui/src/lib/i18n/semantic.ts`، القاموسان أعلاه، `apps/ui/tests/wire-values.test.ts`.
- **النهج:** قواميس أسماء من vocabulary الذي يُصدره `crates/pc-intelligence/src/coordinator.rs` و`crates/diagnostic-engine/src/lib.rs`؛ native `<details>` عند الحاجة؛ [HTML details](https://html.spec.whatwg.org/multipage/interactive-elements.html#the-details-element).
- **الخطوات:** (1) احصر IDs الفعلية ببحث ضيق في المصدر المذكور، دون نسخ نصوص سجل. (2) عرّف map typed لكل collector/provider وUnknown collector للمستقبلي. (3) اعرض «فحص صحة التخزين — تعذّر الوصول» بدل `hardwareTelemetry`. (4) لا تعرض ID حتى في technical disclosure العادي؛ اجعله ضمن نسخة تشخيص محلية اختيارية فقط، مع عدم تضمين النص الخام في accessible name.
- **قبول:** fixture يضم كل مصدر معروف ومصدرًا مجهولًا وحالات timeout/denied/cancelled؛ headings/buttons لا تحتوي IDs في اللغتين، وغياب جامع لا يولد Healthy. لا يتغير routing أو code المرسل للسيرفر.
- **المخاطر/القيود:** فقدان ربط الدعم بالجامع؛ السجل المحلي يحتفظ بالهوية. H3,H4,H6.

### P77-03 — بوابة تمنع عودة التسرب [INFERRED، 200–350 سطر]

- **الملفات — MODIFY:** `apps/ui/tools/verify-arabic.mjs`، `apps/ui/tools/layout-sweep.mjs`، `apps/ui/src/dev/layout-fixture.ts`، `apps/ui/tests/wire-values.test.ts`، `.github/workflows/ci.yml`.
- **النهج:** compiler الموجود لـSvelte، Node test runner الموجود، متصفح أداة sweep الموجودة؛ [Svelte compiler](https://svelte.dev/docs/svelte/svelte-compiler). لا regex على ملفات HTML الخام كدليل وحيد.
- **الخطوات:** (1) fixture يحوي فشل IPC ونصًا خامًا مجهولًا وعمليات مكتملة/جزئية ونتيجة صفر. (2) افحص text nodes والـattributes الميسّرة؛ التقنية المسموحة = قيم بيانات محددة وموسومة عند المصدر، لا استثناء عام لكل `TechnicalText`. (3) افحص literal UI prose ومراجع المفاتيح واختلاف placeholders وplural branches EN/AR؛ اسم منتج معروف مسموح صراحة، لا سماح لأي Latin. (4) شغّل gate في CI بـfixture، مع negative control يزرع نصًا في label وآخر في attribute وآخر داخل argument. (5) حافظ على بوابات seal/freeze.
- **قبول:** الحقن الثلاثي يفشل gate، وكذلك مفتاح ناقص/placeholder مخالف؛ النسخة المصححة تمر في EN/AR وdark/light و720/960/1280، دون truncation للأسماء؛ فشل تحميل fixture يُفشل gate بدل صفر صفحات ناجحة.
- **المخاطر/القيود:** لا يمكن إثبات لغة أي اسم يرسله الجهاز؛ الضمان ميكانيكي لكل **نص يملكه التطبيق** وحدود عرض البيانات. محتوى المستخدم/الجهاز موسوم ومعزول وليس translated prose. H4,H6.

### P77-04 — أخطاء Fleet والخدمة مفاتيح لا جمل [INFERRED، جلستان مستقلتان A/B، كل منهما ≤300 سطر]

- **A الملفات — MODIFY:** `services/maintenance-service/src/errors.rs`، `apps/ui/src/features/shared.ts`، `apps/ui/src/platform/service-client.ts`، `apps/ui/tests/wire-values.test.ts`، القاموسان. **B — MODIFY:** `apps/desktop/src/main.rs` (مواضع `fleet_*` فقط)، `apps/ui/src/features/fleet/FleetPage.svelte`، `apps/ui/tests/fleet.test.ts`، القاموسان.
- **النهج:** `common.proto:ErrorInfo` الموجود (message_key/args/correlation_id/technical_detail)؛ [Tauri commands/errors](https://v2.tauri.app/develop/calling-rust/). لا invent رسالة جديدة لكل `error.to_string()`.
- **الخطوات لكل جلسة:** تتبع producer→transport→renderer؛ اجمع الحالات المميزة (timeout/offline/denied/invalid/unavailable)؛ اربطها بمفاتيح معروفة؛ اجعل raw detail للسجل فقط، وplaceholder بيانات bounded. في B لا تغيّر توقيع Fleet إن كان ذلك يغير عقد desktop؛ اعرض fallback P77-01 لحين قرار المالك.
- **قبول لكل جلسة:** أخطاء timeout وpermission وفشل جزئي ونص مجهول تصل بالعربية، ولا يُعرض failure كنجاح؛ يظل error code كما هو ويتاح correlation id؛ اختبارات مستهلكي EN لا تفقد السبب المصنف.
- **المخاطر/القيود:** **OWNER DECISION D1** فقط إذا استلزم B تغيير شكل عقد desktop أو إذا نقص حقل في wire؛ التوصية reuse `ErrorInfo` أولًا، ثم إضافة typed code ضيقة. H4,H5,H6. **MEASURED:** ledger `DBT-P76-002` يظل open؛ نطاق B يحتاج تتبعًا قصيرًا في التنفيذ ولا يفترض أن كل Fleet خطأ خام.

## §2 P78 — تقييم إصلاح ينتهي ويفسّر ما يفعل

**MEASURED:** `system-repair/src/lib.rs:159–167` لا يمرر control أو progress لـ`assess`. `:340–346` يأخذ `completed=now_ms()` **قبل** `platform.assess()`، والعامل لا يحول panic إلى terminal state. `windows_impl.rs:39–89` يجري ScanHealth ثم SFC ثم WUA ثم فحص القرص متتابعة. `dism_api.rs:DismCheckImageHealth` يمرر cancel=0/progress=None. `windows-update/src/windows_impl.rs:59–63` يستعمل Online=true وSearch متزامنًا في health probe، بخلاف scope الصريح في اكتشاف التعريفات. السبب الدقيق لتعليق جهاز المستخدم **INFERRED** حتى التسجيل؛ لا نعلن أن DISM وحده سببه.

### P78-01 — health probe محلي وتاريخ اكتمال صحيح [INFERRED، 150–250 سطر]

- **MODIFY:** `crates/windows-update/src/windows_impl.rs`، `crates/windows-update/src/lib.rs`، `crates/system-repair/src/lib.rs`، `crates/system-repair/tests/coordinator.rs`.
- **النهج/مرجع:** [IUpdateSearcher properties](https://learn.microsoft.com/en-us/windows/win32/wua_sdk/iupdatesearcher-properties)؛ reuse `SearchScope::LocalCacheOnly`؛ اختبار fake platform وclock محكوم داخل الاختبار.
- **الخطوات:** احصر callers لـ`probe_update_health`؛ اجعله محليًا صراحة، مع نص يصف cache لا آخر تحديثات الإنترنت؛ لا Online fallback عند نقص cache. انقل completed timestamp إلى ما بعد نهاية العمل. حوّل panic/spawn error إلى failed assessment محفوظ ومربوط بنفس assessment_id، مع عدم تحرير gate لمزوّد ما زال يعمل.
- **قبول:** اختبار search scope يرفض online لكل استدعاء تقييم محلي؛ empty cache=Unknown أو «لم يُفحص حديثًا»، لا «Windows محدث»؛ fake assessment يتقدم وقته فتكون completed≥نهاية القياس؛ panic لا يترك Scanning. probe شبكة على Windows لاحقًا يثبت عدم اتصالات **من هذا المسار**، ولا يُنسب نشاط Windows المستقل للتطبيق.
- **المخاطر/القيود:** **OWNER DECISION D2 — touches network:** التوصية إزالة online الافتراضي الآن وعدم إضافة online بديل هنا. لا يتغير وصول WUA الرسمي بعد موافقة تثبيت صريحة. H1,H3,H6.

### P78-02 — دورة تقييم محدودة وتحديث كل check [INFERRED، 250–350 سطر]

- **MODIFY:** `crates/system-repair/src/lib.rs`، `crates/system-repair/src/windows_impl.rs`، `crates/system-repair/tests/coordinator.rs`؛ تعديل callers في `crates/pc-intelligence/src/sources.rs` إذا تغيرت الواجهة الداخلية. لا ملفات أخرى دون إثبات caller.
- **النهج:** `collector-runtime:CancellationToken/CollectorControl/IsolationGate` الموجودة؛ [DISM CheckImageHealth](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism/dismcheckimagehealth-function).
- **الخطوات:** (1) أضف control/emit داخليين إلى `RepairPlatform::assess` وحدّث fake platform. (2) حدّث `checks` بعد كل خطوة مع id ثابت؛ لا تنتظر نهاية الجميع. (3) احتفظ بموعد نهائي مستقل لكل مزوّد، واجعل timeout نتيجة Unknown لذلك الجزء مع سبب، وassessment terminal عندما تنتهي محاولة الجمع. (4) لكل عامل generation/id fence يمنع late write فوق تقييم أحدث. (5) لا تبدأ عاملًا ثانيًا إذا لم يعد الـAPI الأول؛ حدّ timeout وحده لا يقتل thread.
- **قبول:** fake أول خطوة ناجحة ثم الثانية لا تعود: الأولى ظاهرة، تصل حالة timeout، ولا تتراكم workers عند الضغط المتكرر؛ نتيجة متأخرة لا تغير assessment الجديد؛ read lease لا يصبح إذن تشغيل متوازٍ لعملية قديمة. tests لا تنتظر مهلات production.
- **المخاطر/القيود:** كشف stuck work بلا تحرير مواردها مبكرًا؛ إن لم تتوقف API احتفظ بحالة busy صادقة وقدّم restart service يدويًا بعد خلو mutation. H1,H3,H5,H6.

### P78-03 — عقد إلغاء التقييم والإصلاح [INFERRED، 150–250 سطر؛ محجوب بقرار]

- **MODIFY:** `crates/contracts/proto/repair.proto`، `crates/contracts/proto/operations.proto`، `services/maintenance-service/src/router/repair.rs`، `services/maintenance-service/src/router/dispatch.rs`، `crates/system-repair/src/lib.rs`؛ اختبارات داخل الملفات. يولّد المنفذ bindings بآلية المشروع المعتمدة، ويسجل ملفات المخرجات في تقريره.
- **OWNER DECISION D3:** إضافة cancel verbs تحمل assessment_id/plan_id؛ التوصية نعم. `CancelRequest` في `events.proto` يلغي RPC لا عملية انتهى RPC الخاص بها. لا إعادة تفسيره.
- **النهج/مرجع:** [DISM cancellation](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism/dismrestoreimagehealth-function)؛ طلب الإلغاء ليس دليل توقف العملية.
- **الخطوات:** احجز tags بعد فحص main؛ gate owner/identity؛ cancel idempotent؛ return الحالة الحالية؛ طلب إلغاء قبل mutation يمنع عبور الحاجز، وبعده ينتظر توقف API ويكتب recovery-needed عند عدم معرفة الحالة. عرّف cancelRequested/cancelled كعرض أو status مدعوم دون إعادة ترقيم enums؛ لا تضف UI قبل تنفيذ الإلغاء الفعلي P85.
- **قبول:** مالك آخر مرفوض، id قديم لا يلغي الجديد، تكرار الطلب آمن؛ late completion لا يتحول إلى success بعد نتيجة غير مؤكدة؛ رفض البروتوكول القديم واضح. cancel لا يحذف journal ولا يعيد تنفيذ command.
- **المخاطر/القيود:** H2,H3,H5,H6؛ D0 وD3 لازمان.

### P78-04 — شاشة تقدّم وتزامن لا تبقى «Assessing» [INFERRED، 150–300 سطر]

- **MODIFY:** `services/maintenance-service/src/streaming.rs`، `apps/ui/src/features/repair/controller.ts`، `apps/ui/src/features/repair/RepairPage.svelte`، `apps/ui/tests/wire-values.test.ts`، القاموسان؛ `apps/desktop/src/main.rs` فقط bindings للـcancel المعتمد.
- **النهج:** ordered events + `HydrateSessionRequest` الموجود، لا polling موازٍ دائم؛ [WAI-ARIA progressbar](https://www.w3.org/TR/wai-aria-1.2/#progressbar)؛ progress غير محدد لا يحمل نسبة مصطنعة.
- **الخطوات:** اعرض الخطوة الجارية، الزمن المنقضي، آخر تحديث ومصدر عدم القياس؛ عند قطع الاتصال اعرض Disconnected ولا تستنتج إلغاء backend؛ hydration يعيد نفس العملية؛ زر الإلغاء يظهر فقط عند توفر P78-03/P85 cancel؛ timeout يعرض إعادة المحاولة إذا gate تسمح. النسبة مأخوذة من callback فقط، وإلا مؤشر غير محدد.
- **قبول:** event terminal يصل قبل start RPC المتأخر فلا يعيده إلى Scanning؛ restart/reconnect يعيد الحالة الصحيحة؛ missing progress لا يعطي 100%؛ keyboard وscreen reader يسمعان تغير المرحلة لا كل tick؛ إلغاء يعود للمستخدم بنتيجة terminal حقيقية.
- **المخاطر/القيود:** H3,H4,H5,H6؛ لا تخفيف بروتوكول replay أو مهلة إظهار فشل زائفة من الواجهة.

## §3 P79 — Care مفهوم ومسجّل

**MEASURED:** `services/maintenance-service/src/care.rs:94–125` يركب **الخطط الموجودة فقط** في ReadyForReview/AwaitingAuthorization؛ هذا ليس مسحًا أو إنشاء خطة. `CarePlan::build(...).unwrap_or_else(...)` يحول فشل البناء إلى خطة فارغة. `timeline-intelligence/src/ingest.rs` يقرأ أربع عائلات ولا يقرأ `care_runs` رغم وجود `Database::care_runs_for_owner`. `timeline.rs:timeline_page_proto/page_for_owner` يعامل `before_sequence=0` كحد نهائي صفر، والـUI يرسله صراحة؛ هذا يكفي لإرجاع أول صفحة فارغة. هذه أسباب كودية مستقلة عن التخمين بشأن تسجيل جهاز المستخدم.

### P79-01 — أول صفحة Timeline ليست فارغة [INFERRED، 80–160 سطر]

- **MODIFY:** `services/maintenance-service/src/timeline.rs`؛ اختبارات unit فيه. `apps/ui/src/features/timeline/controller.ts` فقط إن كان الخطأ بحاجة عرض أفضل.
- **النهج:** `timeline.proto:GetTimelinePageRequest` ينص أن zero يعني newest. لا API Windows جديدة؛ [Protocol Buffers presence/defaults](https://protobuf.dev/programming-guides/field_presence/).
- **الخطوات:** أضف timeline من ثلاث أحداث واختبر request `(page_size=2,before=0)`؛ حوّل zero إلى `len` قبل القص؛ حافظ على cursor للأقدم وحدود الحجم؛ افصل service failure عن no events في UI إن لزم. لا تعالج concurrent pagination بتغيير العقد هنا.
- **قبول:** أحدث حدثين يظهران، cursor يعيد الأقدم بلا تكرار، empty صحيح، huge size bounded، owner isolation محفوظ. الطلب الحالي يعيد صفرًا قبل الإصلاح، وبعده صفحتين صحيحتين.
- **المخاطر/القيود:** cursor الحالي index يتأثر بتغير النافذة؛ يُعالج P87-02. H3,H5,H6.

### P79-02 — إدخال Care من سجله الدائم [INFERRED، 150–250 سطر]

- **MODIFY:** `crates/timeline-intelligence/src/ingest.rs`، `crates/timeline-intelligence/tests/ingestion.rs`، `apps/ui/src/features/timeline/TimelinePage.svelte`، القاموسان.
- **النهج:** reuse `care_runs_for_owner` و`care_steps_for_run` وTimelineEvent؛ لا نسخ أحداث mutable إلى جدول ثانٍ.
- **الخطوات:** أضف mapper لكل run terminal مع source_id=`care:<run_id>` ووقت النهاية المحفوظ، outcome مستمد من تحقق الخطوات لا حالة Completed وحدها؛ cancellation=neutral مع label مستقل؛ failed واضح. لا تجعل رسالة «نجاح جزئي» نجاحًا موثقًا. bounded reads؛ لا تضاعف run مع كل تحديث للخطوات. UI يسمي المجال والنتيجة ويتيح معرفة ما تغيّر.
- **قبول:** run محفوظ ثم coordinator جديد يرى صفًا واحدًا؛ completed-unverified لا يصبح Verified؛ cancelled/failed محفوظان؛ ingestion لنفس run مرتين لا يكرر الحدث، وتشغيلان مختلفان يحتفظان بهويتين؛ بيانات مالك B لا تظهر لـA؛ child domain events تبقى ولا تُحسب Care كتكرار عطل جديد.
- **المخاطر/القيود:** semantics dedup والrecurrence؛ اجعل مصدر-run مختلفًا عن نمط القصور في المجالات. H3,H4,H5,H6؛ D0 فقط إن احتجت حقولًا جديدة، ويمكن إنجاز الإدخال بالعقد الحالي.

### P79-03 — اشرح الخطة الفارغة ولا تبتلع خطأ بناءها [INFERRED، 150–250 سطر]

- **MODIFY:** `services/maintenance-service/src/care.rs`، `apps/ui/src/features/care/CarePanel.svelte`، `apps/ui/src/features/care/controller.ts`، `apps/ui/tests/care.test.ts`، القاموسان.
- **النهج:** existing `summary_key` + حالات ثابتة؛ no Windows API جديدة، نفس القواعد من `care-orchestrator/src/model.rs`.
- **الخطوات:** انشر خطأ `CarePlan::build` بدل empty fallback؛ سمِّ absence بدقة «لا توجد خطط آمنة جاهزة»؛ رابط إلى Scan/Review المناسب، لا «جهازك لا يحتاج شيئًا». أظهر summary خارج شرط وجود steps. ميّز Loading/Unavailable/No prepared plans وReviewOnly، ولا تُظهر Approve صفر أعمال.
- **قبول:** over-cap أو خطة invalid=error، لا nothingDue؛ scan نظيف وخطة غير محضرة وفشل قاعدة البيانات ثلاث تجارب مختلفة؛ review-only لا يُنفّذ؛ لا grant/start عند zero auto steps؛ اختبار digest وسingle-use القديم يظل ناجحًا.
- **المخاطر/القيود:** H2,H3,H4,H6؛ **OWNER DECISION D4** لأي تعديل سياسة الموافقة؛ التوصية لا تغييرها هنا.

### P79-04A — تحضير Care من نتائج قراءة محلية [INFERRED، 250–350 سطر؛ owner-gated]

- **MODIFY:** `services/maintenance-service/src/care.rs`، `services/maintenance-service/src/router/care.rs`، `services/maintenance-service/src/router/dispatch.rs`، `crates/contracts/proto/care.proto`، `crates/contracts/proto/operations.proto`؛ اختبارات داخل `care.rs`.
- **OWNER DECISION D5:** verb مستقل PrepareCarePreview يقرأ/يحضّر فقط، يعيد سبب أهلية كل مجال ووقت الفحص؛ التوصية نعم. لا توسيع Auto إلى إصلاح أو تعريفات أو تعطيل خدمات.
- **الخطوات:** اعتمد طلبًا صريحًا يبدأ فحص cleanup المحلي أولًا؛ لا تنادِ deep scan الذي قد يسلك بحث تعريفات online قبل P84. انتظر بحدود P78، واستخدم API إنشاء خطة cleanup الحالي للمرشحين منخفضي الخطر فقط؛ لا mutation. أعد preview frozen digest وreasons (notScanned/stale/none/reviewRequired/unavailable). لا تجعل started scan بمفرده يوافق على deletion.
- **قبول:** user temp آمن يعطي preview غير فارغ بلا حذف؛ لا network calls؛ locked/unavailable يذكر السبب؛ نتيجة قديمة لا تتحول لخطة جديدة بلا إعادة فحص؛ عرض preview لا يكتب consent؛ private owner scope.
- **المخاطر/القيود:** freshness وTOCTOU يعاد التحقق منهما عند domain barrier. H1,H2,H3,H5,H6؛ D0,D5.

### P79-04B — رحلة «افحص ← راجع ← وافق ← نتيجة» [INFERRED، 150–250 سطر، بعد A]

- **MODIFY:** `apps/desktop/src/main.rs` (command binding فقط)، `apps/ui/src/features/care/controller.ts`، `apps/ui/src/features/care/CarePanel.svelte`، `apps/ui/src/app/PlanDialogs.svelte`، `apps/ui/tests/care.test.ts`، القاموسان.
- **النهج:** existing digest-bound grant؛ [Tauri commands](https://v2.tauri.app/develop/calling-rust/).
- **الخطوات:** زر Care يطلب التحضير ثم يعرض ما سيُحذف وحجمه وما استُبعد ولماذا؛ التأكيد الحالي يوافق على bytes المعروضة فقط؛ النتيجة تعرض verified/skipped/failed والمساحة الفعلية من المجال، وزر تفاصيل يفتح Activity/Timeline. لا تجمع «نقرة واحدة» مع موافقة ضمنية.
- **قبول:** happy path بعد الموافقة فقط؛ plan drift يعيد Review، رفض المالك=صفر mutation؛ no candidates يعرض تغطية وسببًا؛ relaunch يعرض run السابق من history؛ EN/AR keyboard flow كامل.
- **المخاطر/القيود:** H2,H3,H4,H6؛ D5 يشمل رحلة consent، لا موافقة متوارثة لتشغيل لاحق.

## §4 P80 — حكم مفهوم مبني على تغطية قابلة للفحص

**MEASURED:** `pc-intelligence/src/model.rs:SystemFact` يحمل freshness/confidence/source/time وtyped payload، و`PcFinding` في العقد يحمل مفاتيح ترجمة وأدلة. `diagnostics.proto` يحمل SMART وNVMe بالفعل؛ لا حاجة لإعادة بناء منظومة تشخيص من الصفر. `performance.proto` يحمل has_temperature، لكن حقول أخرى رقمية بلا presence؛ استخدم fault الموجود حتى يوافق المالك على أي توسيع. **INFERRED:** الوصول لجودة HWiNFO لا يعني محاكاة كل حساس؛ نعد بقياس مدعوم وبحدود صريحة، ولا نضيف kernel driver أو raw MSR/EC/SMBus وصولًا غير موثق تحت اسم التشخيص.

### P80-01 — بطاقة verdict وتغطية واحدة في Hardware وDeep Scan [INFERRED، 200–300 سطر]

- **MODIFY:** `apps/ui/src/features/diagnostics/HardwarePage.svelte`، `apps/ui/src/features/intelligence/FindingCard.svelte`، `apps/ui/src/features/intelligence/headline.ts`، `apps/ui/tests/wire-values.test.ts`، القاموسان.
- **النهج:** البيانات الحالية فقط، native disclosure؛ [WCAG use of color](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html).
- **الخطوات:** لكل نتيجة اعرض: ماذا رصدنا، متى/خلال أي نافذة، المصدر، درجة اليقين، ما لم نقسه، والخطوة التالية. مثال: «سُجل خطأ ذاكرة مصحّح مرتان خلال 30 يومًا؛ لا يحدد هذا وحده شريحة تالفة». اعرض no issues in measured checks بدل «كل الجهاز سليم»؛ Partial/Unavailable ليست Healthy؛ افصل Critical عن Denied بلون وشكل ونص. لا health score 0–100 يجمع مجالات غير قابلة للجمع.
- **قبول:** all-unavailable/partial/stale/zero الحقيقي لها نتائج متميزة؛ رقم ثقة ليس probability مخترعًا؛ كل action يقود لمسار مناسب ولا ينفذ mutation؛ EN/AR و200% zoom والملاحة بلوحة المفاتيح.
- **المخاطر/القيود:** حمل بصري؛ مستوى أول جملتان، الباقي details. H3,H4,H6.

### P80-02A — عقد قياسات الإضافات المحددة فقط [INFERRED، 200–300 سطر؛ قرار]

- **MODIFY:** `crates/contracts/proto/diagnostics.proto`، `crates/hardware-telemetry/src/lib.rs`، `crates/crash-diagnostics/src/lib.rs`، `crates/diagnostic-engine/src/lib.rs` (الـpublic structs/defaults فقط).
- **OWNER DECISION D6:** typed additive messages لمجالات ThermalZone/Battery/Boot/Network وcoverage (source, observed time/window, availability/reason key)، وحقول NVMe spareThreshold عند الحاجة. التوصية نعم؛ لا general plugin framework أو generic arbitrary JSON bag.
- **النهج/المرجع:** [Protobuf explicit field presence](https://protobuf.dev/programming-guides/field_presence/)؛ optional قياس غائب مختلف عن صفر مقاس، و[تطور schema](https://protobuf.dev/programming-guides/proto3/#updating) بلا إعادة استعمال tags.
- **الخطوات:** optional measurement لا zero sentinel؛ enum availability محدود مع Unknown للمستقبل؛ limits (مثل 32 حسّاسًا/بطارية، 128 adapter، آخر 20 boot) وbyte budget يثبتها الاختبار؛ الهوية display name منفصلة عن stable id؛ أضف `serde(default)` للقديم دون افتراض قياس. احفظ design/design capacity منفصلًا عن current، وhistorical maximum منفصلًا عن rated threshold.
- **قبول:** decode snapshot قديم لا يصنع حساسات بقيمة صفر؛ empty vs unsupported vs denied مختلفة؛ unknown enum آمن؛ serialization بأقصى أحجام مضبوطة، وround trip 128-bit storage counters بلا فقدان. tags القديمة ثابتة.
- **المخاطر/القيود:** migration للـJSON الدائم وowner scope؛ H1,H3,H4,H5,H6، D0,D6.

### P80-02B — توصيل العقد إلى الخدمة والعرض [INFERRED، 150–300 سطر، بعد A]

- **MODIFY:** `services/maintenance-service/src/protocol.rs`، `apps/desktop/src/main.rs` (تحويل diagnostics فقط)، `apps/ui/src/platform/stream-state.ts`، `apps/ui/src/dev/layout-fixture.ts`، `apps/ui/tests/wire-values.test.ts`؛ output types المعتمدة عبر المولد/مصدرها الحقيقي فقط. **NEW:** `apps/ui/src/features/diagnostics/MeasurementRows.svelte` داخل مجلد diagnostics القائم.
- **النهج:** فصل presence في التحويل end-to-end؛ نفس protobuf docs في P79-01.
- **الخطوات:** مرّر unknown/presence/time بلا default صحي؛ component صغير يعرض الاسم والوحدة/القيمة/الحالة، لا business rules؛ قيود العناصر في producer قبل serialization؛ fixture مأخوذ من أشكال العقد الحقيقي؛ لا تشغيل providers الجديدة بعد.
- **قبول:** source→proto→desktop→UI يحافظ على null و0 منفصلين؛ malformed input لا يكسر الصفحة؛ لا IDs خام؛ fixture ب32 قياسًا لا overflow. الصورة وحدها ليست قبولًا.
- **المخاطر/القيود:** كثرة ملفات glue؛ cap 300 وإذا زاد افصل desktop/UI عن service. H3,H4,H6.

### P80-03 — freshness والنتائج المفقودة لا تُغلق Findings [INFERRED، 200–300 سطر]

- **MODIFY:** `crates/pc-intelligence/src/normalize.rs`، `crates/pc-intelligence/src/lifecycle.rs`، `crates/pc-intelligence/tests/scenarios.rs`؛ `model.rs` فقط إن احتاجت freshness policy الحالية تعديلًا محددًا.
- **النهج:** reuse resolution_evidence/authority الموجودة؛ لا API Windows جديدة.
- **الخطوات:** حافظ على وقت المصدر لا وقت عرض الصفحة؛ ميّز historical finding عن current signal؛ resolution يحتاج إعادة قياس ناجحة لنفس resource من authority المعنية، لا اختفاء الجامع. hardware trend لا يقارن قرصًا جديدًا بهوية القرص القديم. قياسات الأداء عمرها نافذتها، جرد الجهاز يبطل عند تبدله، Event Log يصرح بنافذته وليس TTL واحدًا لكل المجالات.
- **قبول:** critical disk ثم unplug/permission denied يبقى unresolved مع unavailable؛ الفحص النظيف الحقيقي يحل finding وفق قواعده؛ snapshot قديم لا يولد current thermal alarm؛ الساعة المرتدة لا تولد عمرًا سالبًا؛ lifecycle الحالي الذي يحقق هذا يُترك ويُستكمل اختباره فقط.
- **المخاطر/القيود:** الإنذارات العالقة؛ expose «لم نعد نتمكن من التحقق» بدل مسحها. H3,H5,H6.

## §5 P81 — عمق التخزين دون ادعاء دعم شامل

**MEASURED:** `hardware-telemetry/src/windows_impl.rs:195–264,304–387,615–790` يجمع `MSFT_PhysicalDisk` و`MSFT_StorageReliabilityCounter`، وATA عبر `SMART_RCV_DRIVE_DATA`، وNVMe health log عبر IOCTL. الـfallback ليس غياب SMART كاملًا؛ الفجوات هي ربط الهوية/التغطية، spare threshold، وتفسير القيم عبر أنواع الأجهزة. `lib.rs:267–370` يصنف uncorrected errors وcritical-warning/media errors وwear وحد الحرارة، ويضيف NVMe unavailable حتى لغير NVMe. **MEASURED (توثيق Microsoft):** `TemperatureMax` هنا الحد الأعلى للتشغيل الطبيعي، وليس أعلى قيمة تاريخية؛ لا «تصححه» بالافتراض المعاكس. [Storage reliability class](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/msft-storagereliabilitycounter).

### P81-01 — اربط physical disk بالـhandle الصحيح [INFERRED، 200–350 سطر]

- **MODIFY:** `crates/hardware-telemetry/src/windows_impl.rs`، `crates/hardware-telemetry/src/lib.rs`؛ tests داخل lib.
- **API:** WMI namespace `ROOT\Microsoft\Windows\Storage`: `MSFT_PhysicalDisk`, `MSFT_Disk`, associations المتاحة؛ `SetupDiGetClassDevsW(GUID_DEVINTERFACE_DISK)`، `SetupDiEnumDeviceInterfaces`، `SetupDiGetDeviceInterfaceDetailW`، `DeviceIoControl(IOCTL_STORAGE_GET_DEVICE_NUMBER)` و`StorageDeviceProperty` للربط. [Device number](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-ioctl_storage_get_device_number)، [NVMe access](https://learn.microsoft.com/en-us/windows/win32/fileio/working-with-nvme-devices).
- **الخطوات:** أوقف افتراض أن `MSFT_PhysicalDisk.DeviceId.parse()` هو دومًا PhysicalDrive index؛ أثبت mapping من device number والهوية، ولا تدمج بمجرد الاسم/السيريال الفارغ. Storage Spaces/RAID/USB: علامة provider limitation إذا لا توجد correspondence مثبتة. Read-only IOCTLs فقط؛ stop عند hot-unplug، لا retry غير محدود. لا expose serial افتراضيًا.
- **قبول:** fixture index مختلف عن DeviceId لا يقرأ قرصًا آخر؛ قرصان بنفس الاسم لا يختلطان؛ USB bridge unsupported لا يصبح healthy؛ live مقارنة device number/size/bus على SATA+NVMe+USB وStorage Spaces إن توفر. عدم توفر جهاز يبقي coverage غير مثبتة.
- **المخاطر/القيود:** D6 إن احتاج wire stable id جديد؛ لا تغيير معنى field منشور بصمت. H1,H3,H5,H6.

### P81-02 — NVMe health كامل الدلالة [INFERRED، 150–300 سطر]

- **MODIFY:** `crates/hardware-telemetry/src/lib.rs`، `crates/hardware-telemetry/src/windows_impl.rs`؛ mapping `services/maintenance-service/src/protocol.rs` فقط للحقول المعتمدة P80.
- **API:** `IOCTL_STORAGE_QUERY_PROPERTY`, `StorageDeviceProtocolSpecificProperty`, `ProtocolTypeNvme`, `NVMeDataTypeLogPage`, `NVME_LOG_PAGE_HEALTH_INFO`؛ [NVME_HEALTH_INFO_LOG](https://learn.microsoft.com/en-us/windows/win32/api/nvme/ns-nvme-nvme_health_info_log).
- **الخطوات:** extend decoder الموجود لـAvailableSpareThreshold، افصل warning bits بمعناها؛ قيمة PercentageUsed قد تتجاوز100، لا clamp إلى صحة صفر ولا convert إلى عمر متبق مضمون؛ درجات الحرارة Kelvin→C عند القيمة الصالحة؛ counter u128 يبقى exact string. قارن cumulative errors بالسابق فقط عند نفس هوية الجهاز وزمن موثق؛ unsafe shutdowns لوحدها ليست disk failure. احفظ return length/offset/checksum constraints الموجودة.
- **قبول:** spare أقل من threshold يعطي Attention، unsupported 0/unknown لا يصبح حرارة −273؛ counters أكبر من2^64 تحفظ؛ buffer قصير/offset overflow/unknown bits لا panic؛ critical-warning يتحول إلى توصية backup قبل أي repair كثيف، لا «إصلاح SSD».
- **المخاطر/القيود:** H3,H4,H6، D6. لا firmware updates أو NVMe self-test ضمن scan.

### P81-03 — ATA وعدم التغطية والترند [INFERRED، 200–300 سطر]

- **MODIFY:** `crates/hardware-telemetry/src/lib.rs`، `crates/hardware-telemetry/src/windows_impl.rs`، `crates/diagnostic-engine/src/lib.rs`؛ tests inline.
- **API:** المصدر الحالي `SMART_RCV_DRIVE_DATA`، ثم `IOCTL_STORAGE_PREDICT_FAILURE` إذا يدعمه الجهاز؛ [failure prediction](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntddstor/ni-ntddstor-ioctl_storage_predict_failure). لا raw-write ATA passthrough.
- **الخطوات:** coverage بحسب bus: ATA لا يفشل لغياب NVMe fields. عرض normalized Current/Worst وraw vendor value كما يبلغه الجهاز، بلا تفسير universality للـraw IDs. استخدم PredictFailure/Windows health للverdict؛ أي threshold table إضافي لا يفسر إلا وفق مواصفة الجهاز المتاحة. اقرأ snapshot سابق من persistence الحالي للdelta لا daemon جديد؛ لا تحتفظ بسلاسل غير محدودة.
- **قبول:** SATA healthy مع غياب NVMe لا يحمل «4 counters missing»؛ missing ATA=unsupported لا healthy؛ counter reset/device replacement لا ينتج delta سالبة أو تحسن مصطنع؛ لا حكم 05/C5/C6 لكل SSD من raw value وحده.
- **المخاطر/القيود:** OEM differences، RAID concealment؛ H1,H3,H6. **OWNER DECISION D7:** أي جدول vendor جديد أو dependency/signed sensor provider مؤجل حتى توجد أجهزة ومرجع وترخيص واختبارات له؛ التوصية عدم إدخاله الآن.

### P81-04 — عرض مخاطر القرص وصحته [INFERRED، 150–250 سطر]

- **MODIFY:** `apps/ui/src/features/diagnostics/HardwarePage.svelte`، `apps/ui/src/lib/i18n/semantic.ts`، `apps/ui/tests/wire-values.test.ts`، القاموسان.
- **النهج:** P80 evidence rows؛ المصادر المذكورة P81-02/03.
- **الخطوات:** لكل قرص اسم/سعة/bus/firmware، verdict، latest reading، wear/temperature/error counters، تفاصيل SMART اختيارية. لا قرص واحد يمثل الجهاز كله. أول إجراء عند evidence data-risk هو backup/recovery guidance؛ لا زر optimizer يكتب على قرص مشكوك فيه. أظهر missing per device، ولا تختصره إلى global healthy.
- **قبول:** ثلاثة أقراص أحدها critical وآخر unsupported يظهران مستقلين؛ no temp=شرطة مع السبب، wear105% يبقى105؛ مصدر threshold واضح؛ AR لا يعرض summary إنجليزيًا عبر interpolations.
- **المخاطر/القيود:** over-alarm من cumulative counters؛ فرّق «سبق أن سجّل» عن «يزداد الآن». H3,H4,H6.

## §6 P82 — الحرارة والطاقة والذاكرة وأعطال النواة

### P82-01 — أوقف نسبة الاختناق إلى حرارة بلا دليل [INFERRED، 120–220 سطر]

- **MEASURED:** `performance-telemetry/src/windows_impl.rs:495–519` يجعل current/max<85% Power throttle، وMhzLimit<100 Thermal، مع أن الوحدة MHz، ولا توجد قراءة حرارة في هذا المسار. السلوك ثابت في اللقطة ولو ذكر AMBITION إصلاحه.
- **MODIFY:** `crates/performance-telemetry/src/windows_impl.rs`، `crates/performance-telemetry/src/lib.rs` (tests/types فقط عند الحاجة).
- **API:** `CallNtPowerInformation(ProcessorInformation)`، [PROCESSOR_POWER_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/power/processor-power-information-str).
- **الخطوات:** استخدم layout الموثق 6 ULONG، والمصفوفة بعدد المعالجات الصحيح؛ لا سبب حراري من idle frequency. إن دل MhzLimit على حد، صفه OS-reported frequency limit؛ cause unknown دون مصدر إضافي. fault field للسبب غير المقاس بدل None التي قد تفهم نفيًا. احتفظ بخصوصية temperature unavailable.
- **قبول:** idle 800/4000MHz لا ينتج Thermal/Power fault؛ MhzLimit=3000 من4000 لا يعامل3000%؛ all-zero/malformed=unavailable؛ لا buffer overrun مع multi-group CPU. قواعد bottleneck لا تعلن سببًا بعد فقدان دليل سببه.
- **المخاطر/القيود:** إزالة alarm خاطئ يمكن أن تقلل عدد findings؛ هذا مقصود. H3,H6؛ D0 إذا تغير wire، وليس مطلوبًا لإيقاف الاستنتاج.

### P82-02 — حرارة مناطق ACPI والطاقة/البطارية [INFERRED؛ جلستان A/B، ≤300 سطر لكل منهما]

- **A — NEW:** `crates/hardware-telemetry/src/thermal.rs`؛ **MODIFY:** `crates/hardware-telemetry/src/lib.rs`، `crates/hardware-telemetry/src/windows_impl.rs`. **B — NEW:** `crates/hardware-telemetry/src/battery.rs`؛ **MODIFY:** الملفان نفسهما. مجلد src قائم، لا crate جديدة.
- **API A:** `IWbemServices::ExecQuery` في `ROOT\WMI`، `MSAcpi_ThermalZoneTemperature` (`InstanceName`, `CurrentTemperature`, `CriticalTripPoint`, `PassiveTripPoint`) إن نشرها firmware؛ discover class/property qualifiers أولًا، finite `IEnumWbemClassObject::Next`. **INFERRED support per machine**؛ لا وثيقة عامة تعد CPU die temperatures. [Windows thermal zones](https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/thermal-management-in-windows).
- **API B:** `GetSystemPowerStatus`؛ enumerate `GUID_DEVINTERFACE_BATTERY` ثم `IOCTL_BATTERY_QUERY_TAG`, `IOCTL_BATTERY_QUERY_INFORMATION`/`BATTERY_INFORMATION`, `IOCTL_BATTERY_QUERY_STATUS`. WMI `ROOT\CIMV2:Win32_Battery` fallback لجرد البطارية فقط عند نقص fields. [Battery information](https://learn.microsoft.com/en-us/windows/win32/power/battery-information-str)، [Win32_Battery](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-battery).
- **خطوات A:** نشر zone name ودرجة وقت القياس مع تحويل أعشار Kelvin؛ لا تسمّ zone «CPU» بلا mapping موثق، و0/unsupported=unavailable؛ لا حد ثابت90 لكل الأجهزة. **خطوات B:** عرض AC/battery presence/charge/design/full/cycles إن كانت مدعومة؛ relative units تظل relative؛ design=0 يمنع division؛ wear نسبة capacity تفسيرية لا فشل مؤكد؛ multi-battery منفصل.
- **قبول A:** 3000 deci-K≈26.85°C؛ missing zone لا صفر؛ firmware ثابت/غير معقول يوصف unreliable؛ timeout bounded. **قبول B:** desktop no-battery طبيعي،0 design غير متاح، relative capacity لا mWh، unplug/tag change يعيد القراءة دون خلط.
- **المخاطر/القيود:** لا تغطية fan RPM/VRM/CPU hotspot عامة عبر WMI. **OWNER DECISION D7** لأي SDK vendor أو kernel dependency؛ التوصية إظهار الحدود بدلها. D6 prerequisite للعرض؛ H1,H3,H5,H6. لا تغييرات power plan أو fan control.

### P82-03A — WHEA من حقول typed لا كلمات مترجمة [INFERRED، 200–350 سطر]

- **MEASURED:** `crash-diagnostics/src/lib.rs:classify_event` يبحث عن memory/cache/pci في concatenated payload؛ هذا ليس CPER decoder.
- **MODIFY:** `crates/crash-diagnostics/src/windows_impl.rs`، `crates/crash-diagnostics/src/lib.rs`. **NEW:** `crates/crash-diagnostics/src/whea.rs` داخل src القائم؛ unit byte fixtures داخله.
- **API:** `EvtQuery/EvtNext/EvtRender` لقناة `System` ومزوّد `Microsoft-Windows-WHEA-Logger`؛ `EvtOpenPublisherMetadata` لschema/version؛ `RawData` CPER عند توفره: `WHEA_ERROR_RECORD_HEADER`, section descriptors, `WHEA_MEMORY_ERROR_SECTION`, processor/PCIe GUIDs. [Error records](https://learn.microsoft.com/en-us/windows-hardware/drivers/whea/error-records)، [Memory section valid bits](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntddk/ns-ntddk-_whea_memory_error_section).
- **الخطوات:** احفظ provider+event id+version+record id/time؛ typed numeric fields أو bounded CPER decoder للتحقق من signature/length/section count/offsets؛ corrected/uncorrected/fatal من severity الفعلية، لا EventID وحده. مواقع DIMM/APIC/BDF تُعرض فقط مع valid bits؛ غير المفكوك=Hardware error of unknown component. لا تحميل كامل log.
- **قبول:** Arabic/English Windows لنفس payload يعطيان نفس category؛ malformed/unknown sections لا panic ولا تخمين DIMM؛ corrected لا «RAM تالفة»، حدث بلا memory flag لا memory fault؛ cap=truncated coverage لا30 يومًا كاملة؛ unreadable=unknown.
- **المخاطر/القيود:** hostile binary buffers وschema drift؛ H3,H5,H6؛ D0 إذا أضيف record id/severity structured للعقد، التوصية في D6 كإضافة صغيرة منفصلة عند التنفيذ.

### P82-03B — RAM pressure ونتائج اختبار الذاكرة [INFERRED، 150–250 سطر]

- **MODIFY:** `crates/crash-diagnostics/src/windows_impl.rs`، `crates/diagnostic-engine/src/lib.rs`، `apps/ui/src/features/diagnostics/HardwarePage.svelte`، القاموسان؛ tests inline.
- **API:** `GlobalMemoryStatusEx` الحالي، `Win32_PhysicalMemory`/`Win32_PhysicalMemoryArray` للجرد وECC capability لا error counts؛ Event Log `System` provider `Microsoft-Windows-MemoryDiagnostics-Results` بمطابقة manifest/version. [Win32_PhysicalMemory](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-physicalmemory)، [Event render](https://learn.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtrender).
- **الخطوات:** فصل memory pressure عن memory hardware errors؛ عرض آخر نتيجة Windows Memory Diagnostic ووقت الاختبار ومدى التغطية؛ لا وجود حدث≠passed؛ لا تشير إلى DIMM إلا بالدليل P82-03A. قدم إرشادًا لفتح `mdsched.exe` مع تحذير reboot وحفظ العمل، لا schedule/reboot آليًا.
- **قبول:** RAM load95% دون WHEA لا hardware fault؛ WHEA memory رغم load20% يظهر؛ missing test=not tested؛ نتيجة قديمة مؤرخة لا شهادة سلامة الآن؛ فتح reboot tool لا يتم من scan.
- **المخاطر/القيود:** **OWNER DECISION D8** إذا أضيف launcher مؤثر على reboot؛ التوصية guided manual فقط. H2,H3,H4,H5,H6.

### P82-04 — Crash/bugcheck بدليل وهوية مشتركة [INFERRED، 200–350 سطر]

- **MODIFY:** `crates/crash-diagnostics/src/windows_impl.rs`، `crates/crash-diagnostics/src/lib.rs`، `crates/diagnostic-engine/src/lib.rs`، `apps/ui/src/features/diagnostics/CrashPage.svelte`، القاموسان.
- **API:** `System` providers `Microsoft-Windows-WER-SystemErrorReporting` (1001 مع schema)، `Microsoft-Windows-Kernel-Power` (41)، `EventLog` (6008 إن لزم). dump metadata الموجودة مع قراءة header bounded؛ [WinDbg kernel dump](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/analyzing-a-kernel-mode-dump-file-with-windbg).
- **الخطوات:** ربط الحدث والdump بcode/time/path normalized مع mismatch ظاهر، لا count مرتين؛ Kernel-Power41 يثبت unexpected shutdown فقط. أضف missing-dump reason (not configured/deleted/unreadable) فقط إذا قيس، وإلا unknown. لا تنسب ntoskrnl.exe أو آخر driver إلى root cause؛ confidence explicit. روابط محلية لـEvent Viewer/WinDbg فقط؛ `.bugcheck`/`!analyze -v` إرشاد لمن يملك الرموز محليًا.
- **قبول:** event+dmp=حادثة واحدة، Event41 وحده لا PSU diagnosis؛ header مجهول لا parser تخميني؛ timestamp fallback لا1970؛ لا قراءة dump كامل/إرساله في support bundle افتراضيًا؛ malformed files لا تكسر الخدمة.
- **المخاطر/القيود:** minidump قد يحتوي أسرارًا. **OWNER DECISION D9:** DbgEng dependency أو symbol download مؤجل؛ التوصية عدم bundling debugger أو auto symbol server. H1,H3,H4,H5,H6.

## §7 P83 — الإقلاع والشبكات وصحة تحديث Windows

**MEASURED:** `pc-intelligence/src/model.rs:FactPayload` لا يمثل boot-duration أو adapter counters أو battery/thermal measurements؛ `sources.rs:DeepScanBackend` يربط driver/repair/diagnostics/startup/cleanup/product updates. لا تُخلط `update-engine` (تحديث AetherCore) بصحة Windows Update. نطاق عدم وجود القياس المثبت هنا هذه المسارات، لا ادعاء أن كل مستودع خالٍ من أي ذكر للشبكات.

### P83-01A — بيانات boot من Event Log [INFERRED، 180–300 سطر]

- **NEW:** `crates/crash-diagnostics/src/boot.rs`؛ **MODIFY:** `crates/crash-diagnostics/src/lib.rs`، `crates/crash-diagnostics/src/windows_impl.rs` (مصدر إضافي محدود).
- **API:** `Microsoft-Windows-Diagnostics-Performance/Operational`, provider `Microsoft-Windows-Diagnostics-Performance`, Event100/101/102/103 وفق metadata المثبتة على builds المدعومة؛ `BootTime/MainPathBootTime/BootPostBootTime/BootStartTime/SystemBootInstance` عندما يثبت schema وجودها. [Publisher metadata](https://learn.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtopenpublishermetadata)، [Windows boot performance](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/on-off-transition-performance).
- **الخطوات:** آخر20 إقلاع بحد وقت/bytes؛ parser مسمّى الحقول لا offsets غير موثقة؛ افصل cold/restart/fast-startup إن سمحت البيانات وإلا النوع unknown؛ لا تعتبر level=Critical في الحدث hardware critical. uptime عبر Win32_OperatingSystem.LastBootUpTime سياق فقط وليس boot duration. لا تُفعّل log معطل بصمت.
- **قبول:** ms→seconds صحيح، event مفقود=unknown، duplicate boot instance لا يُكرر؛100 يحمل boot duration لا وقت تشغيل الجهاز؛غياب schema لا قراءة عشوائية. Windows fixture يستخرج من machine manifest، لا community sample كعقد.
- **المخاطر/القيود:** الأسماء/النسخ **INFERRED** حتى probe على Windows؛ H1,H3,H5,H6، D6.

### P83-01B — أثر startup من قياسات الإقلاع [INFERRED، 150–250 سطر]

- **MODIFY:** `crates/startup-manager/src/lib.rs`، `crates/startup-manager/src/windows_impl.rs`، `apps/ui/src/features/startup/StartupPage.svelte`، القاموسان؛ tests داخل lib.
- **API:** providers P83-01A؛ جرد `Run/RunOnce` في HKCU/HKLM، Startup folders، `ITaskService`/Task Scheduler، `EnumServicesStatusExW` القائم؛ [Task Scheduler](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-scheduler-start-page).
- **الخطوات:** لا تُعد كثرة startup entries مدة boot؛ اربط delay event بملف/خدمة مؤكدة وإلا «غير منسوب». median لخمسة boots متماثلة النوع إن توفرت، مع current والbaseline window، ولا causal improvement من تجربة واحدة. retain protected entries/reversible plans الموجودة.
- **قبول:** same display name لبرنامجين لا يدمج؛ newly installed app بلا measured delay=unknown impact؛ boot تحسن بلا دليل attribution لا يعلن توفيرًا من تعطيل التطبيق؛ لا disable تلقائي.
- **المخاطر/القيود:** owner paths حساسة؛ H2,H3,H4,H5,H6. **OWNER DECISION D10** إن احتاج wiring dependency بين startup وcrash أو حقول impact جديدة؛ التوصية تمرير typed snapshot من composition الحالية بدل crate coupling.

### P83-02 — Adapter diagnostics محلي بالكامل [INFERRED؛ جلستان A/B، ≤300 سطر]

- **A — NEW:** `crates/hardware-telemetry/src/network.rs`؛ **MODIFY:** `crates/hardware-telemetry/src/lib.rs`، `crates/hardware-telemetry/src/windows_impl.rs`. **B — MODIFY:** `crates/diagnostic-engine/src/lib.rs`، `apps/ui/src/features/diagnostics/HardwarePage.svelte`، القاموسان، `apps/ui/tests/wire-values.test.ts`.
- **API:** `GetAdaptersAddresses(AF_UNSPEC)`، `GetIfTable2/GetIfEntry2` (`MIB_IF_ROW2` link speed/oper status/InErrors/OutErrors/InDiscards/OutDiscards/Octets)، `GetIpForwardTable2` للdefault routes؛ `ROOT\StandardCimv2:MSFT_NetAdapter` optional metadata وليس shell parser. [Addresses](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getadaptersaddresses)، [GetIfEntry2](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getifentry2).
- **خطوات A:** bounded buffer/retry وRAII؛ link identity=GUID/LUID ضمن session، reset عند adapter change؛ counters cumulative تُحوّل delta بوقت حقيقي فقط؛ لم تتوفر عينتان=not measured. **خطوات B:** افصل adapter disabled/cable disconnected/APIPA/no gateway عن «الإنترنت معطّل»؛ VPN/virtual adapters تسمّى ولا تُصنّف تلقائيًا fault؛ لا reverse DNS للعنوان ولا ping أو socket ضمن scan.
- **قبول A:** counter reset/wrap لا spike؛ hot unplug ومئات adapters لا overflow؛ صفر transmission حقيقي يختلف عن failed read. **قبول B:** IPv6-only/VPN-only لا critical كاذب؛ no gateway لا يثبت internet failure؛ network capture يثبت لا probes صادرة من الفحص.
- **المخاطر/القيود:** MAC/IP/SSID تبقى محلية وتخضع للredaction عند export. **OWNER DECISION D11:** أي اختبار DNS/TCP/HTTP اتصال مستقل بوجهة وكمية/timeout معلنين بعد موافقة؛ التوصية خارج التنفيذ الأول. H1,H3,H4,H5,H6، D6.

### P83-03A — Windows Update history والفشل المتكرر [INFERRED، 200–300 سطر]

- **MODIFY:** `crates/windows-update/src/lib.rs`، `crates/windows-update/src/windows_impl.rs`، `crates/system-repair/src/windows_impl.rs`؛ tests inline.
- **API:** `IUpdateSearcher::GetTotalHistoryCount/QueryHistory`, `IUpdateHistoryEntry` date/operation/result/HResult/update identity؛ `ISystemInformation::RebootRequired`؛ `QueryServiceStatusEx/QueryServiceConfigW` لـwuauserv/BITS/TrustedInstaller. [QueryHistory](https://learn.microsoft.com/en-us/windows/win32/api/wuapi/nf-wuapi-iupdatesearcher-queryhistory)، [Update troubleshooting](https://learn.microsoft.com/en-us/troubleshoot/windows-server/installing-updates-features-roles/troubleshoot-windows-server-update-guidance).
- **الخطوات:** history محلية بصفحات صغيرة حتى30 يومًا/200 حدث؛ distinguish query unavailable from zero؛ newest success لنفس update/revision يغلق failed attempt وليس failures لأخرى؛ service demand-start stopped ليست corruption؛ pending reboot advisory. لا تشغل service لغرض probe ولا تضبط سياسات WSUS.
- **قبول:** فشل ثم نجاح لنفس identity لا يعرض unresolved؛ فشلان متكرران يظهران بالتواريخ والHRESULT؛ history empty لا «آخر تحديث مثبت»؛ WUA cache offline لا يذهب online؛ stopped trigger-start لا يولد repair candidate وحده.
- **المخاطر/القيود:** history ناقصة بعد reset، لا استنتاج patch compliance من آخر نجاح. H1,H3,H6؛ **OWNER DECISION D2** تغطي السياسة المحلية.

### P83-03B — دليل update event وربطه بالواجهة [INFERRED، 150–300 سطر]

- **MODIFY:** `crates/crash-diagnostics/src/windows_impl.rs`، `crates/diagnostic-engine/src/lib.rs`، `apps/ui/src/features/repair/RepairPage.svelte`، القاموسان؛ tests inline.
- **API:** `Microsoft-Windows-WindowsUpdateClient/Operational` + System provider نفسه، read named fields وفق event version؛ المرجع السابق و`EvtOpenPublisherMetadata` P83-01A.
- **الخطوات:** evidence-only bounded event subset للفشل/الreboot؛ لا hardcode دلالة ID من forum ولا تخلط channelين؛ أظهر Windows Update health منفصلًا عن AetherCore update؛ خيار فتح Windows Settings لإجراء صاحب الجهاز، دون `UsoClient` undocumented commands أو reset شامل.
- **قبول:** unavailable channel لا success؛ provider+id+version mismatch يصبح unknown؛ event/historical record لنفس محاولة لا يضاعف failure count؛ AR keys لكل verdict؛ لا egress جديد.
- **المخاطر/القيود:** H1,H3,H4,H6؛ أي فتح صفحة قد يقود لonline action يذكره المستخدم ويظل اختياريًا D11.

### P83-04 — ETW جلسة دعم اختيارية، لا tracing دائم [INFERRED، خطة قياس مؤجلة بقرار]

- **MODIFY عند اعتماد التنفيذ:** `crates/performance-telemetry/src/lib.rs`، `crates/performance-telemetry/src/windows_impl.rs`؛ **NEW:** `crates/performance-telemetry/src/etw_capture.rs`. جلسة300 سطر أولى تثبت lifecycle/cancel فقط، لا محلل لكل provider.
- **OWNER DECISION D12:** التوصية إبقاء read-only Event Log/PDH الوضع الافتراضي، وإتاحة capture محلية زمنها60ثانية بموافقة واضحة إذا بقيت مشكلة أداء غير مفسرة. لا new dependency دون D0/H6.
- **API:** `StartTraceW/EnableTraceEx2/OpenTraceW/ProcessTrace/ControlTraceW(EVENT_TRACE_CONTROL_STOP)`؛ providers المرشحة `Microsoft-Windows-Kernel-Process`, `Microsoft-Windows-Kernel-Disk`, `Microsoft-Windows-Kernel-Processor-Power`, `Microsoft-Windows-DxgKrnl`. تُكتشف keywords/versions بـ`TdhEnumerateProviders/TdhEnumerateManifestProviderEvents`؛ لا enable-all. [ETW sessions](https://learn.microsoft.com/en-us/windows/win32/etw/configuring-and-starting-an-event-tracing-session).
- **الخطوات:** تثبيت manifest المحدد أولًا، session اسم فريد، caps time/bytes، only essential events، تشغيل صريح وإغلاق RAII عند cancel/fault، marker dropped-events؛ لا تسجل command lines أو payload network. أعطِ raw local trace لصاحب الجهاز مع حالة القياس، ولا يبني النموذج سببًا من event rate وحدها.
- **قبول:** cancel/service stop يوقف فقط جلسة AetherCore، cap يمنع نمو الملف، dropped events=partial، capture failure لا يغير جلسة WPR لغيره، no network أو symbol download.
- **المخاطر/القيود:** overhead وخصوصية process paths؛ H1,H2,H3,H5,H6. هذا اختيار دعم لاحق، ليس شرطًا لتسليم P77–P83-03.

### P83-05A — توصيل الحرارة والبطارية إلى Hardware [INFERRED، 150–250 سطر، بعد P80-02/P82-02]

- **MODIFY:** `crates/diagnostic-engine/src/lib.rs`، `services/maintenance-service/src/protocol.rs`، `apps/ui/src/features/diagnostics/HardwarePage.svelte`، `apps/ui/src/dev/layout-fixture.ts`، `apps/ui/tests/wire-values.test.ts`، القاموسان.
- **النهج:** reuse snapshot owner-scoped وMeasurementRows من P80؛ APIs الحرارية/البطارية وروابط Microsoft محددة في P82-02، لا polling جديد في UI.
- **الخطوات:** انقل مجموعتي القياسات المحدودتين عبر snapshot أثناء scan الحالي؛ attach sample timestamp/coverage؛ اعرض الأقسام عند دعمها وسببًا موجزًا عند عدم الدعم. لا private IDs ولا نسبة صحة للبطارية عندما الوحدات relative. لا تشغل القراءة مع كل render.
- **قبول red→green:** fake provider=zone26.85°C/battery relative يصلان عبر protocol والعرض كما هما؛ denied/no battery مختلفان؛ AR labels والاسم الميسّر بلا raw prose؛ snapshot owner آخر لا يمر.
- **المخاطر/القيود:** producer وحده لا يغلق الميزة؛ هذا اختبار الوصل الضروري. H1,H3,H4,H5,H6؛ ضمن D6 المعتمد، أي توسعة إضافية D0.

### P83-05B — توصيل سجل الإقلاع إلى Startup [INFERRED، 150–250 سطر، بعد P83-01]

- **MODIFY:** `crates/diagnostic-engine/src/lib.rs`، `services/maintenance-service/src/protocol.rs`، `apps/ui/src/features/startup/StartupPage.svelte`، `apps/ui/src/dev/layout-fixture.ts`، `apps/ui/tests/wire-values.test.ts`، القاموسان.
- **النهج:** Event Log boot evidence وروابط Microsoft في P83-01؛ snapshot المشترك الحالي، دون جعل startup inventory يعيد Event Log query مستقلة.
- **الخطوات:** اعرض آخر successful measured boots بنوع cold/resume/fast-startup إن ثبت؛ baseline لنفس نوع الإقلاع فقط؛ وضّح فترة القياس وعدد العينات. اربط أثر تطبيق بعينة attribution فعلية فقط، وإلا «غير مقاس»؛ لا مدة تقديرية من عدد startup entries.
- **قبول:** اختلاف cold/resume لا ينتج regression كاذب؛ 3 عينات متجانسة تعرض median الصحيح؛ entry غير مرتبط بدليل لا يحمل milliseconds؛ snapshot قديم=historical لا current. fixture EN/AR يغطي غياب قناة الأحداث.
- **المخاطر/القيود:** log clearing/retention لا يعني boot سريعًا. H3,H4,H5,H6؛ D6، وأي consent لتعطيل startup يبقى خارج هذا العرض.

### P83-06 — قواعد findings للقياسات الجديدة [INFERRED؛ جلستان P83-06A/P83-06B، ≤250 سطر لكل منهما]

- **MODIFY لكل جلسة:** `crates/pc-intelligence/src/model.rs`، `crates/pc-intelligence/src/normalize.rs`، `crates/pc-intelligence/src/rules.rs`، `crates/pc-intelligence/tests/scenarios.rs`، القاموسان. **A:** thermal/battery فقط. **B:** boot/network فقط.
- **MEASURED:** `normalize.rs` و`rules.rs` يستهلكان FactPayload typed؛ إضافة sensor fields وحدها لا تضيف حكمًا منضبطًا. **النهج:** توسيع القائمة الحالية بلا rule engine جديد؛ APIs ومراجع العتبات لكل مصدر في P82/P83، ولا Windows call داخل rules.
- **الخطوات A:** threshold حراري من firmware الموثوق فقط؛ battery wear معلومة تفسيرية لا critical alarm تلقائي. **B:** boot regression مقابل baseline مماثل؛ adapter errors بدلالة delta/window لا accumulated counter تاريخي؛ cable disconnected لوحده حالة لا عطل شامل للإنترنت. أضف evidence id/time/source والحدود إلى finding؛ unknown لا healthy. لا استنتاج causal link بين حرارة وتعطل أو بين startup item وبطء من التزامن وحده.
- **قبول A:** missing/rated threshold وold sample لا thermal alarm؛ supported exceeded trip يحمل الدليل والوحدة. **قبول B:** counter reset/down adapter/first sample لا rate كاذب؛ boot mixed types لا regression. لكل جلسة red case قبل الإضافة، round-trip finding بترجمة كاملة، وإعادة قياس ناجحة فقط تسمح resolution.
- **المخاطر/القيود:** precision قبل كثرة التنبيهات؛ المنتج لا يحتاج finding لكل قيمة. H3,H4,H6؛ أي FactPayload serialized wire جديد يخضع D0/D6، لا schema tricks.

## §8 P84 — دورة تعريفات رسمية، محددة السلطة، قابلة للاسترداد

**MEASURED:** `windows-pnp/src/lib.rs:DeviceRecord/DeviceStatus/InstalledDriver` يغطي الجرد والـproblem code. `windows-update/src/lib.rs:SearchScope` يفصل passive local عن online، ونسخة الهدف المستخرجة من العنوان موسومة TitleHeuristic. `driver-install/src/lib.rs:InstallPlatform,run execution` يستدعي restore-point وdriver-backup قبل WUA. `driver-backup/src/lib.rs` يعرّف SHA256 manifest، و`restore-point/src/lib.rs:RestorePointEvidence` يحمل verified_fresh. **INFERRED:** لا نكتب updater جديدًا؛ نغلق سياسة البحث والتحقق والواجهة فوق الموجود.

### P84-01 — جدول Inventory يشرح problem code ولا يخمن outdated [INFERRED، 150–300 سطر]

- **MODIFY:** `crates/windows-pnp/src/windows_impl.rs`، `crates/windows-pnp/src/lib.rs`، `apps/ui/src/features/drivers/DriversPage.svelte`، القاموسان، `apps/ui/tests/wire-values.test.ts`.
- **API:** `SetupDiGetClassDevsW(DIGCF_PRESENT|DIGCF_ALLCLASSES)`، `SetupDiEnumDeviceInfo`, `SetupDiGetDevicePropertyW` (`DEVPKEY_Device_DriverVersion/DriverDate/DriverProvider/DriverInfPath/ProblemCode/ProblemStatus/DevNodeStatus`)؛ reuse `CM_Get_DevNode_Status`. [Device status/problem](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/retrieving-the-status-and-problem-code-for-a-device-instance).
- **الخطوات:** افحص fallback failure في كل field بدل تحويلها إلى0؛ اعرض missing Code28، disabled22 كاختيار مستخدم محتمل، failed-start10، stopped43، signature52 بمعناها لا تعليمات «حدّث الجميع». unknown code عنوان عام مع رقم في details. oldest date ليس outdated؛ outdated لا يُكتب إلا candidate رسمي applicable مع مقارنة موثوقة. label «عرض Windows تحديثًا» عندما version مجهولة.
- **قبول:** API failure لا device healthy؛22 لا urgent update،28 مختلف عن10؛ تعريف أقدم تاريخًا وأعلى rank لا auto downgrade؛ no offers+offline cache لا up-to-date؛ جميع الحالات EN/AR.
- **المخاطر/القيود:** **OWNER DECISION D13** لحقل ProblemStatus/presence إذا يحتاج wire، التوصية additive في `drivers.proto` بمهمة عقد منفصلة ≤150سطر قبل عرضه؛ لا تغيير هيكل الجرد أثناء المهمة دونها. H3,H4,H5,H6.

### P84-02A — بحث online بنية مستخدم صريحة [INFERRED، 200–300 سطر؛ قرار]

- **MODIFY:** `crates/contracts/proto/drivers.proto`، `services/maintenance-service/src/router/drivers.rs`، `crates/driver-hub/src/lib.rs`، `crates/windows-update/src/windows_impl.rs`، `crates/pc-intelligence/src/sources.rs`؛ tests داخل hub.
- **OWNER DECISION D14:** إضافة search scope/consent intent واضح لطلب البحث؛ التوصية المحلي هو default حتى للطلب القديم الفارغ، وonline لا يُقبل إلا بعد تأكيد يوضح Windows Update/الخادم المدبّر. scope enum وحده ليس owner authentication: router يربطه بالمالك والنية المحددة.
- **API:** `IUpdateSession::CreateUpdateSearcher`, `IUpdateSearcher::Online=false/true`, `CanAutomaticallyUpgradeService=false`, `ServerSelection` الحالي وفق سياسة الجهاز؛ [WUA properties](https://learn.microsoft.com/en-us/windows/win32/wua_sdk/iupdatesearcher-properties). لا `AddService2` للالتحاق بخدمة جديدة ولا تجاوز WSUS.
- **الخطوات:** enumerate كل caller للبحث (scheduler/deep scan/repair/UI/CLI)؛ default local؛ online intent يستعمل مرة، expire ومربوط scan؛ network failure لا repeated background retry؛ جمع cached inventory لا يتوقف على network. ضع الاختبار على المسار الحقيقي لا fake لا يُستخدم.
- **قبول:** scheduler وفتح الواجهة وdeep scan وrepair يعبرون صفر online calls؛ user confirmed search يمر مرة؛ caller B لا يستخدم نية A؛ رفض dialog=صفر؛ timeout لا يترك online worker يستأنف بعد cancel.
- **المخاطر/القيود:** H1,H2,H5,H6، D0,D14. حماية pipe لا تُخفف حتى لتسهيل query.

### P84-02B — بحث WUA قابل للإنهاء وعرض مصدر البيانات [INFERRED، 200–300 سطر]

- **MODIFY:** `crates/windows-update/src/windows_impl.rs`، `crates/windows-update/src/lib.rs`، `apps/ui/src/features/drivers/controller.ts`، `apps/ui/src/features/drivers/DriversPage.svelte`، `apps/desktop/src/main.rs` (binding scope فقط)، القاموسان؛ tests inline.
- **API:** `IUpdateSearcher::BeginSearch`, `ISearchJob::RequestAbort`, `EndSearch`، COM apartment lifetimes؛ [BeginSearch](https://learn.microsoft.com/en-us/windows/win32/api/wuapi/nf-wuapi-iupdatesearcher-beginsearch).
- **الخطوات:** bounded async job، terminal acknowledgment بعد انتهاء job/abort، لا تدمير callback مبكرًا؛ شاشة تتمايز فيها Inventory/Local cached offers/Online as of timestamp. زر مستقل «البحث عبر Windows Update» وتأكيد EN/AR يذكر الاتصال. EULA/متطلبات download لا تُقبل صامتة.
- **قبول:** fake job لا يكتمل → abort مرة ثم unavailable/busy حتى يتوقف؛ no COM callbacks بعد free؛ offline search يعرض cache age؛ UI لا تظهر newest/latest من cache؛ factory search path يمر negative no-egress test.
- **المخاطر/القيود:** WUA قد يتباطأ بعد abort؛ لا promise فوري ولا zombie concurrency. H1,H3,H4,H6؛ D14.

### P84-03 — التوقيع وهوية الحزمة والتحقق من downgrade [INFERRED، 200–350 سطر]

- **MODIFY:** `crates/driver-authority/src/lib.rs`، `crates/driver-authority/src/truth.rs`، `crates/driver-install/src/lib.rs`، `crates/driver-install/tests/coordinator.rs`، `crates/windows-update/src/execution_windows.rs` (preflight فقط).
- **API:** WUA `IUpdateIdentity(UpdateID,RevisionNumber)`, `IWindowsDriverUpdate` applicability، per-update `IInstallationResult`؛ للـINF المحلي مستقبلًا `SetupVerifyInfFileW` مع catalog وplatform، `WinVerifyTrust`/catalog member hashes لا توقيع .sys وحده. [SetupVerifyInfFile](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupverifyinffilea)، [WinVerifyTrust](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrust).
- **الخطوات:** request install مرتبط device identity/current INF/hash/offer id+revision؛ refresh قبل mutation والتحقق من unchanged expected state. WUA يدير trusted installation؛ لا تعرض Cryptographically Verified badge لأن URL رسمي فقط. بعد التنفيذ افحص actual bound INF/version/problem code وreboot؛ مقارنة رقمية components لا lexicographic ولا title heuristic proof. إذا actual version أقدم وغير مقصودة: verification failure/recovery review، لا auto-force rollback.
- **قبول:** WUA succeeded لكن old INF باقٍ لا Verified؛ code43 بعد update لا Healthy؛ revision تغيّر بعد consent مرفوض؛ title يقولversion9 لكن boundversion1 يبقىunverified؛13.0>9.9 رقميًا؛ partial install لا success شامل.
- **المخاطر/القيود:** Windows قد يفضل older version أعلى rank؛ هذا يحتاج قرار المستخدم لا وصفه خطأ قطعيًا. **OWNER DECISION D15:** أي مسار يسمح intentional downgrade/changes consent، التوصية خارج Auto وموافقة منفصلة. WinVerifyTrust نجاحه `0` فقط؛ offline cache-only trust لا يجلب CRL، نقص revocation يبقى unknown، لا «توقيع سليم حديثًا». H1,H2,H3,H6.

### P84-04 — حماية قابلة للإثبات قبل Install [INFERRED، 200–300 سطر]

- **MODIFY:** `crates/driver-backup/src/lib.rs`، `crates/driver-backup/src/windows_impl.rs`، `crates/restore-point/src/windows_impl.rs`، `crates/driver-install/tests/coordinator.rs`؛ production coordinator فقط إن كشف الاختبار غياب الحاجز.
- **API:** `SRSetRestorePointW` begin/end؛ `ROOT\DEFAULT:SystemRestore` لإثبات sequence جديد؛ `pnputil.exe /export-driver oemN.inf <protected-local-dir>`؛ [Restore point](https://learn.microsoft.com/en-us/windows/win32/api/srrestoreptapi/nf-srrestoreptapi-srsetrestorepointw)، [PnPUtil](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/pnputil-command-syntax).
- **الخطوات:** reuse existing protection، تحقق من fresh sequence لا return-code فقط؛ تحقق exported INF/CAT/member file manifest nonempty/hash-readback، disk capacity، protected product root وعدم reparse؛ validate handles عند re-read. Inbox INF not exportable → explicit BackupNotApplicable وشرح أن restore point ليس نسخة بيانات. protection failure يمنع BeginInstall، لا خيار «تجاهل» ضمن Care.
- **قبول:** restore disabled/frequency-throttled/stale sequence، disk full، export0 files، hash drift، directory replacement: كلها تمنع mutation؛ checkpoint موجود قبل before_install؛ crash بعد barrier يُسجّل recovery-required. existing green protection tests لا تعاد كتابتها، أضف السيناريو الناقص فقط.
- **المخاطر/القيود:** restore point لا يضمن استعادة ناجحة ولا بيانات المستخدم؛ **OWNER DECISION D16** لأي تغيير safety tier أو bypass، التوصية رفضه. H2,H3,H5,H6.

### P84-05 — استرداد موثّق ومسار Catalog يدوي [INFERRED، 150–250 سطر]

- **MODIFY:** `apps/ui/src/features/RecoveryPanel.svelte`، `apps/ui/src/features/drivers/DriversPage.svelte`، القاموسان، `apps/ui/tests/wire-values.test.ts`.
- **API/نهج:** عرض backup manifest/path/restore sequence من الحقول الموجودة وإرشاد Device Manager Roll Back؛ [DiRollbackDriver](https://learn.microsoft.com/en-us/windows/win32/api/newdev/nf-newdev-dirollbackdriver) يملك Windows backup واحدًا وقد يعيد ERROR_NO_MORE_ITEMS؛ [Microsoft Update Catalog](https://www.catalog.update.microsoft.com/Home.aspx) عبر browser فقط بعد طلب صريح.
- **الخطوات:** سمِّ «ملفات التصدير موجودة» لا «يمكن الاستعادة حتمًا»؛ تحقق أن rebind السابق يحتاج OS support؛ pnputil /add-driver يمكنه staging/install بأفضل ranking ولا يفرض downgrade، فلا تقترح delete/force للتغلب عليه. Catalog رابط يدوي مع نسخة hardware id بإجراء صريح، لا scraping/download في الخلفية؛ أي package يدوي يجب أن يمر فحص signing/architecture/HWID ثم خطة consent جديدة قبل تسليمه للنظام.
- **قبول:** backup غير متاح لا زر rollback مزيف؛ reboot-required واضح؛ official link لا يفتح تلقائيًا أو يضم serial/IP؛ unknown recovery يظهر guidance لا succeeded؛ زيارة Catalog لا تعتبر installed.
- **المخاطر/القيود:** **OWNER DECISION D17:** إن أراد المالك rollback آليًا أو importer لحزم Catalog، فهما مرحلتان لاحقتان بعقد وموافقة مستقلين؛ التوصية WUA execution + manual recovery الآن. H1,H2,H3,H4,H6.

**INFERRED — خارج النطاق الملزم للتعريفات:** لا third-party driver feeds، لا تنزيل OEM EXE أو GPU utilities تلقائيًا، لا firmware/BIOS/UEFI flashing، لا /force delete-driver، لا تغيير Secure Boot/test-signing/HVCI، لا replacing boot-critical/storage/network drivers تحت Care، لا ادعاء إكمال inventory يعني معرفة أحدث تعريف لكل جهاز. هذه حدود جودة وأمان، وليست backlog مخفيًا.

## §9 P85 — إصلاح مدعوم وتنظيف قابل للفهم

**MEASURED:** `system-repair/src/windows_impl.rs:380–398` يستدعي RestoreHealth بلا `/LimitAccess`. `:459–505` يقرأ tail CBS القديم ويعامل وجود أي `[SR]` بلا كلمات خطأ كـNoViolation؛ لا يربط ببدء العملية. `:560–648` timeout لchild مع stdout/stderr لا progress callback. `RepairPlatform` الحالي يشغّل mutation/verify وexisting atomic journal موجود؛ يحتفظ به التنفيذ.

### P85-01 — إثبات SFC من نافذة العملية لا من ذيل قديم [INFERRED، 150–250 سطر]

- **MODIFY:** `crates/system-repair/src/windows_impl.rs`، `crates/system-repair/src/lib.rs` (pure parser للاختبار)، `crates/system-repair/tests/coordinator.rs`.
- **API/نهج:** `sfc.exe /verifyonly`/`/scannow` في System32 الموثوق، File ID/offset/mtime baseline لـCBS؛ [SFC logs](https://learn.microsoft.com/en-us/troubleshoot/windows-client/installing-updates-features-roles/analyze-sfc-program-log-file-entries)، [SFC command](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/sfc).
- **الخطوات:** احفظ هوية CBS وطوله ووقت البدء؛ اقرأ الجزء الجديد bounded، وتعامل مع rotation/truncation. لا تفترض أن markers CBS public stable verdict API؛ parser حذر مع complete run evidence، والأجزاء الناقصة/جلسة servicing أخرى=Unknown. لا return NoViolation لمجرد غياب cannot repair. أبقِ subsequent verify-only بعد repair ومراجعة result code.
- **قبول:** old clean log + current corrupt → ليسHealthy؛ old corrupt + fresh clean لا يظل corrupt بسبب الماضي؛ `[SR]` progress فقط=Unknown؛ rotation/permission error لا success؛ nonzero/timeout لا verified حتى مع clean tail قديم.
- **المخاطر/القيود:** CBS schema/parallel OS servicing؛ عند عدم attribution ارفض الحكم لا ترفع الثقة. H3,H5,H6.

### P85-02 — DISM progress/CancelEvent وoffline source [INFERRED، 250–350 سطر]

- **MODIFY:** `crates/system-repair/src/dism_api.rs`، `crates/system-repair/src/windows_impl.rs`، `crates/system-repair/src/lib.rs`، `crates/system-repair/tests/coordinator.rs`.
- **API:** `DismInitialize/OpenSession/CheckImageHealth/RestoreImageHealth/CloseSession/Shutdown`، progress callback(current,total)، `CreateEventW/SetEvent` cancel، `LimitAccess=TRUE`. [DismRestoreImageHealth](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism/dismrestoreimagehealth-function).
- **الخطوات:** reuse FFI module وتأكد ABI من SDK المثبت؛ callbacks لا panic/lock database، coalesce updates؛ عند total0 لا percentage. check cancellation/deadline من P78؛ cancel=state unknown حتى verification جديدة، لا Completed. LimitAccess=true الافتراضي، source missing=SourceRequired، no implicit Windows Update؛ attach exit/HRESULT typed. لا concurrent DismShutdown/session mutation.
- **قبول:** callbacks 0/0,40/100,100/100 تنتج unknown,40,100 للعملية الحالية فقط؛ success of API لا bypass verification؛ timeout يشير cancel event مرة ولا يحرر servicing lock قبل عودة API؛ cancel بعد mutation=recovery-needed؛ missing source لا اتصال ولا إعادة محاولة تلقائية.
- **المخاطر/القيود:** Dism cancellation قد يترك image unknown؛ لا قتل TrustedInstaller. **OWNER DECISION D18 — network/consent:** التوصية local-only restore؛ خيار online repair source مستقبلي يحتاج موافقة واضحة منفصلة وحجم/وجهة وصلاحية نية، ولا يندمج مع Care. H1,H2,H3,H5,H6، D3.

### P85-03 — تقدّم SFC وإيقافه بأمان [INFERRED، 200–350 سطر]

- **MODIFY:** `crates/system-repair/src/windows_impl.rs`، `crates/system-repair/src/lib.rs`، `crates/system-repair/tests/coordinator.rs`، `apps/ui/src/features/repair/RepairPage.svelte`، القاموسان.
- **النهج:** stdout/stderr bounded incrementally؛ `CreateProcessW`/الـCommand الحالي مع process ownership صحيح، لا shell expansion؛ [GenerateConsoleCtrlEvent](https://learn.microsoft.com/en-us/windows/console/generateconsolectrlevent)، [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects). SFC لا يوفر documented cancellation/progress callback مثل DISM.
- **الخطوات:** parse نسبة رقمية إذا بثها SFC في build/locale مثبت، UTF16/encoding وحدود split chunks، ولا parse localized verdict. إن لم يبث نسبة: مرحلة+elapsed+indeterminate هي التقدم الصادق. طلب cancel يمنع أي خطوة تالية فورًا؛ تجربة Windows مع owned process group لتحديد دعم cooperative Ctrl-Break. إن لم يستجب أثناء servicing: «سيقف بعد الخطوة الحالية»، لا Cancelled؛ لا kill-wide أو claim فوري. عدّل timeout الحالي الذي يقتل child دون معرفة CBS إلى توقف محفوظ وverification-required وفق نتيجة التجربة.
- **قبول:**12 ثم35 ثم100 من chunks متعددة لا corrupt encoding؛ no output لا stuck UI؛ cancel لا يبدأ verify/repair التالي؛ child stopped لكن TrustedInstaller ما زال active لا lease released كأن كل شيء انتهى؛ unknown outcome لا Successful. سلوك cancel الحقيقي يُثبت في Windows VM، لا fake فقط.
- **المخاطر/القيود:** **OWNER DECISION D19:** اعتماد wording «إيقاف عند أول نقطة آمنة» عند عدم دعم الإلغاء؛ التوصية هذه، لا force termination تحت اسم safe cancel. H2,H3,H4,H5,H6.

### P85-04 — قائمة إصلاح ضيقة تستند إلى diagnosis [INFERRED، 150–300 سطر]

- **MODIFY:** `crates/windows-repair-intelligence/src/diagnosis.rs`، `crates/windows-repair-intelligence/src/planner.rs`، `crates/system-repair/src/windows_impl.rs`، `crates/system-repair/tests/coordinator.rs`.
- **API:** DISM/SFC أعلاه؛ `QueryServiceStatusEx/QueryServiceConfigW` قبل fixed `StartServiceW(wuauserv)` الموجود؛ `FSCTL_IS_VOLUME_DIRTY`/`chkdsk /scan` للتشخيص مع عدم جدولته تلقائيًا؛ `reagentc /info` لإثبات WinRE configuration بدل وجود exe. [Chkdsk](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/chkdsk)، [REAgentC](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/reagentc-command-line-options).
- **الخطوات:** أنشئ جدول action→specific fact→prerequisites→consent→postcheck؛ service manual/trigger start stopped وحده لا عطل، policy-disabled لا تغيير. WinRE absent يوجّه للحماية اليدوية؛ disk physical errors توجّه backup قبل repair؛ `/f`/`/r` وإصلاح offline/reboot إرشادات مراجعة مستقلة. لا reset registry/network/Winsock/DNS cache/SoftwareDistribution افتراضيًا، ولا تغيير service start type لإجبار النجاح.
- **قبول:** missing diagnosis لا executable node؛ stopped service healthy demand-start لا restart؛ confirmed required-service failure يعطي action محدد؛ unsupported WinRE state لا fabricated recovery-ready؛ graph cycle/changed fingerprint يرفض التنفيذ؛ explicit owner consent لكل mutation.
- **المخاطر/القيود:** **OWNER DECISION D20** لأي action إضافي أو تغيير consent/safety، التوصية إبقاء القائمة القائمة وتصحيح eligibility أولًا. H1,H2,H3,H5,H6.

### P85-05 — حدود فئات التنظيف وعرض الاستثناءات [INFERRED، 200–300 سطر]

- **MEASURED:** `cleaner/src/windows_impl.rs:77–187` يشمل WindowsTemp (48h)، profile temp (7days/review)، shader cache وتقارير/dumps؛ الـcleanup يحمل frozen evidence وreparse guards أصلًا.
- **MODIFY:** `crates/cleaner/src/windows_impl.rs`، `crates/cleaner/src/lib.rs`، `crates/cleaner/tests/coordinator.rs`، `apps/ui/src/features/cleanup/CleanupPage.svelte`، القاموسان.
- **النهج:** reuse allowlist وhandle-based frozen file evidence؛ [GetFileInformationByHandleEx](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandleex). لا DLL cleaner جديدة.
- **الخطوات:** تصنيف صريح: stale temp فقط eligible-after-consent؛ shader cache اختياري يذكر recompilation/stutter؛ WER/crash dumps logs review-only ويُستثنى ما يرتبط بحادثة unresolved؛ لا Downloads/Documents/browser passwords/cookies/history أو registry أو WinSxS/Windows.old/driver-store. لا Recycle Bin auto: `SHEmptyRecycleBin` يحذف عناصر أضيفت بعد preview ولا يطابق frozen set. إذا أُريد مستقبلًا فله consent مستقل بلا ادعاء exact file preview. لا فتح ملفات مستخدم آخر اعتمادًا على صلاحية الخدمة؛ reuse OS-resolved owner root.
- **قبول:** locked/changed file=skipped مع سبب، hardlink/reparse/outside-owner لا deletion؛ dump مرتبط بمشكلة مستبعد؛ no candidate يذكر scanned/excluded/unavailable؛ reclaimed bytes حقيقة بعد التنفيذ لا estimate. زر Select all لا يشمل review-only دون اختيار منفصل.
- **المخاطر/القيود:** permanent deletion بلا undo حقيقي؛ لا تعد باستعادة temp. **OWNER DECISION D21:** تغيير eligibility/retention destructive يحتاج قبولًا؛ التوصية لا فئات جديدة قبل هذه الضوابط. H2,H3,H4,H5,H6.

**INFERRED — ما يستحق الإرشاد لا التنفيذ التلقائي:** Windows Update troubleshooter/Get Help، in-place repair من مصدر Microsoft مطابق، memory diagnostic، hardware vendor support وbackup/recovery. لا bloatware remover عام، ولا registry cleaner، ولا EmptyWorkingSet كـRAM repair، ولا مسح evidence لتقليل عداد المشاكل.

## §10 P86 — ذكاء محلي يستحق الثقة ويمكن العثور عليه

**MEASURED — الأساس:** `services/maintenance-service/src/intelligence.rs:100` يبني evidence pack من maintenance history وrecurrence patterns، لا من جميع أسطح التشخيص المذكورة في التعليقات. `crates/intelligence-core/src/assistant.rs:229–288` يقبل النص إذا بقي استشهاد صالح واحد؛ حذف marker غير صالح لا يحذف الادعاء المحيط به. `AssistantDrawer.svelte` يعرض provisional prose قبل اكتمال grounding. `NavigationRail.svelte` يحتوي زر المساعد واختصار Ctrl+/ بالفعل؛ المشكلة اكتشافه وسياق استعماله، لا غياب الزر. `load_in_background` وlocale في العقد موجودان في P76؛ لا يعاد بناؤهما.

### P86-01 — سد ثغرة الاستشهاد الواحد [INFERRED، 150–250 سطر]

- **الهدف:** لا يُعرض جواب مختلط فيه ادعاء بلا دليل لمجرد وجود `[E1]` في مكان آخر.
- **MODIFY:** `crates/intelligence-core/src/assistant.rs`، `crates/intelligence-core/tests/adversarial.rs`، `apps/ui/src/features/assistant/AssistantDrawer.svelte`.
- **النهج:** تشديد gate الحالي؛ [مبدأ عدم الثقة في خرج النموذج](https://genai.owasp.org/llmrisk/llm052025-improper-output-handling/). parsing ليس برهانًا دلاليًا؛ هذه سدّة أولى تسبق P86-02، ولا تُعلن بعدها semantic grounding مكتملًا.
- **الخطوات:** أضف مثالًا «الخطة اكتملت [E1]. القرص سيتعطل غدًا [E999].» وآخر بلا marker للجملة الثانية. ارفض جوابًا يحتوي مرجعًا مجهولًا بدل إزالة marker وحده؛ اشترط إسناد كل وحدة ادعاء، وارفض النص الملتبس بدل تخمين حدود الجمل العربية. أوقف عرض provisional text في drawer؛ اعرض حالة توليد محلية فقط حتى terminal gate. حافظ على cancellation وfocus trap وعودة التركيز.
- **قبول red→green:** المثالان لا يظهر منهما جواب نهائي؛ streaming الخبيث لا يصل DOM/accessibility tree؛ جواب موثق قصير EN/AR يبقى قابلًا للعرض؛ timeout/cancel لا يترك نصًا غير معتمد. لا يصنّف الاختبار وجود marker كدليل على صحة المعنى.
- **مخاطر/قيود:** ارتفاع الرفض؛ fallback صادق أفضل من جواب مؤذٍ. H3,H4,H6؛ لا wire/dependency change.

### P86-02A — حقائق معتمدة قبل الصياغة [INFERRED، 200–300 سطر]

- **MODIFY:** `crates/intelligence-core/src/model.rs`، `crates/intelligence-core/src/assistant.rs`، `crates/intelligence-core/src/engine.rs`، `crates/intelligence-core/tests/adversarial.rs`.
- **النهج:** سجل صغير داخلي من propositions typed، يستعمل enums وserde الموجودين؛ [JSON parsing في serde](https://serde.rs/). لا vector DB ولا نموذج حكم ثانٍ ولا dependency.
- **الخطوات:** أضف proposition id + evidence id/surface + معنى محدد مثل PlanCompleted/PlanCancelled/RepeatedFailure + قيم typed. يولّد النموذج اختيار IDs وترتيبها من allowlist فقط؛ parser يرفض duplicate/unknown/extra free-text fields. اعرض الحقائق بقوالب EN/AR مملوكة للمنتج، لا paraphrase حرًا قد يقلب completed إلى stopped. ضع إرشادات عامة مسموحة مسبقًا ومنفصلة لفظيًا عن الحقائق. لا إطلاق أوامر أو tool calls من النص. إن لم يلائم السؤال حقائق معتمدة، أجب بقالب «لا تتوفر أدلة كافية».
- **قبول:** pack فيه completed لا يستطيع output مولّد تحويله إلى cancelled ولو استشهد بنفس الدليل؛ رقم/جهاز/مالك غير موجود يُرفض؛ حقن تعليمات داخل evidence لا يوسّع allowlist؛ fixtures عربية تشمل النفي والأرقام العربية/اللاتينية. محاكاة اختيار صحيح تنتج المعنى نفسه EN/AR.
- **المخاطر/القيود:** سقف مقصود للتعبير الحر مقابل ضمان H3؛ توسيع قائمة propositions يأتي فقط مع evidence جديد. **OWNER DECISION D22:** التوصية اعتماد مخرجات fact-template للحقائق بدل وعد غير قابل للإثبات بصحة النثر الحر؛ إذا احتاجت IDs إلى wire فـD0 أولًا. H1,H3,H4,H6.

### P86-02B — ربط الحقائق بالـbackend الحالي [INFERRED، 180–300 سطر، بعد A]

- **MODIFY:** `crates/intelligence-core/src/llama.rs`، `services/maintenance-service/src/intelligence.rs`، `crates/intelligence-core/tests/embedded_generation.rs`، `crates/intelligence-core/tests/offline_boundary.rs`.
- **النهج:** prompt محدود وparser المعتمد في A عبر backend الحالي؛ لا تغيير نموذج أو runtime. مصدر API هنا `crates/intelligence-core/src/model.rs` وواجهات reasoner المقروءة، لا وعد بأن constrained decoding مدعوم دون فحص bindings.
- **الخطوات:** غيّر طلب النموذج إلى schema صغير من IDs؛ clamp عددها حسب pack؛ افشل مغلقًا على خرج غير قابل للتحليل، ثم deterministic fallback من نفس الحقائق. حافظ على deadline وRAM/token caps؛ لا ترفعها لإخفاء latency. مرّر locale الموجود فعلًا إلى القوالب النهائية. لا تسمح لـraw `detail` بتحديد حالة الحقيقة.
- **قبول:** نموذج fake يعيد prose/unknown IDs → fallback موثق؛ JSON صالح → قوالب صحيحة؛ تشغيل embedded model على Windows في جلسة التنفيذ يختبر EN/AR وcancel/no-egress، ولا يصبح شرطًا بطيئًا لكل unit CI. fixture وحدها لا تثبت جودة النموذج الحقيقي.
- **مخاطر/قيود:** معدل schema rejection للنموذج الصغير يجب تسجيله محليًا في تقرير القبول؛ لا تنزيل بديل تلقائي. H1,H3,H4,H6؛ أي تبديل artifact/اعتماديات **OWNER DECISION** مستقل، التوصية تأجيله.

### P86-03 — إضافة أدلة تشخيص حديثة ذات مالك [INFERRED، جلستان A/B، ≤300 سطر لكل منهما]

- **A MODIFY:** `services/maintenance-service/src/intelligence.rs`، `services/maintenance-service/src/assistant.rs`، `services/maintenance-service/src/composition.rs`. **B MODIFY:** `services/maintenance-service/src/intelligence.rs`، `crates/intelligence-core/src/model.rs`. اختبارات inline في الملف المعدّل.
- **MEASURED:** `DiagnosticsCoordinator::snapshot_for_owner` و`history` موجودان في `crates/diagnostic-engine/src/lib.rs:538–563`؛ استعملهما بدل snapshot عام. EvidenceSurface يتضمن أسطحًا أكثر من التي يجمعها composer فعليًا.
- **النهج:** DI للقراءة من coordinators الحالية فقط؛ [SQLite bound parameters](https://www.sqlite.org/c3ref/bind_blob.html) في queries الموجودة عند الحاجة؛ لا فحص جديد استجابة لكل سؤال.
- **A الخطوات/القبول:** مرّر أحدث diagnostics snapshot المسموح للمالك إلى composer نفسه الذي يشترك فيه insights/assistant؛ اختر عددًا محدودًا من الحقائق الحرجة ثم warnings؛ stale/failed provider لا ينتج proposition «سليم». اختبار pack قبل التغيير يفتقد diagnostic حديثًا، وبعده يحتويه؛ owner B لا يرى A، والعينة القديمة لا تُعرض كحالية.
- **B الخطوات/القبول:** أضف repair diagnosis الحالية والنتيجة النهائية لـCare من persistence/قراءات coordinator المحمية، ضمن cap الحالي، مع timestamps وstate typed. لا جلب بلا حد ولا نص سجل كامل. اختبار completed care + unresolved repair يولّد حقيقتين صحيحتين؛ partial/cancelled لا يصبح success؛ لا تُسقط ملاحظة خطر حديثة لصالح 8 صفوف history قديمة.
- **المخاطر/القيود:** هذه جلسات توصيل، لا تعد بإضافة كل EvidenceSurface دفعة واحدة؛ أي API owner-scoped ناقص يُضاف في متابعة مصغرة في مصدره بعد التحقق من أحدث main. H3,H5,H6؛ حقول wire جديدة D0، الداخلية لا تحتاج عقدًا جديدًا.

### P86-04 — مساعد مرئي عند الحاجة ولغته صحيحة [INFERRED، 180–300 سطر]

- **MODIFY:** `apps/ui/src/app/AppShell.svelte`، `apps/ui/src/components/NavigationRail.svelte`، `apps/ui/src/features/assistant/controller.ts`، `apps/ui/src/features/assistant/AssistantDrawer.svelte`، `apps/ui/src/features/intelligence/FindingCard.svelte`، القاموسان. **NEW:** `apps/ui/tests/assistant.test.ts` في tests القائم؛ أضفه صراحةً إلى command الاختبارات في `.github/workflows/ci.yml`.
- **النهج:** إعادة استعمال drawer الحالي، زر واضح باسم «اسأل عن هذه النتيجة» يفتح السؤال المقترح القابل للتعديل؛ [WAI dialog focus/keyboard](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/). لا route جديدة ولا onboarding قسري.
- **الخطوات:** أبقِ زرًا نصيًا ظاهرًا في التنقل لا icon-only؛ contextual entry من finding ذي evidence في pack فقط. أظهر loading/disabled/deadline/insufficient evidence بقوالب عربية؛ لا وعد جاهزية قبل تحميل النموذج. عند تغيير اللغة ألغِ turn النشط أو امنع نشر لغته القديمة، وامسح transcript المرئي السابق أو اعرض طلبًا لإعادة توليده؛ لا ترجمة آلية لجملة مخزنة.
- **قبول:** keyboard وscreen reader يجدان الزر، focus يعود لمصدره، الهاتف/RTL لا يخفيه؛ switch إلى AR أثناء stream لا ينشر terminal EN؛ سؤال سياقي لا يدّعي أن finding موجود في pack إن كان غائبًا؛ fake service cancellation يُختبر دون نموذج حقيقي.
- **مخاطر/قيود:** لا تُرسل question للموديل تلقائيًا بمجرد فتح الصفحة؛ تفاعل المستخدم يبدأ السؤال. لا network أو تنفيذ من assistant. H1,H3,H4,H6.

### P86-05 — عزل cache باللغة وقياس latency محليًا [INFERRED، 150–250 سطر]

- **MEASURED:** `EphemeralInsights` في `services/maintenance-service/src/intelligence.rs` مفهرس بالمالك، و`list(owner)` لا يحدد locale؛ الطلب نفسه يستقبل locale. هذا مصدر محتمل لإعادة عرض جواب سابق بلغة أخرى، لا إثبات runtime أنه حدث.
- **MODIFY:** `services/maintenance-service/src/intelligence.rs`، `apps/ui/src/features/intelligence/controller.ts`، `crates/intelligence-core/src/engine.rs`؛ اختبارات inline. بعد D23 فقط: `crates/contracts/proto/insights.proto`، ومحوّل الطلب في `services/maintenance-service/src/protocol.rs` و`apps/desktop/src/main.rs` إذا لزم؛ افصل wire glue إلى جلسة D23 مستقلة إن تجاوز مجموعها السقف.
- **النهج:** cache key داخلي owner+locale+evidence generation حيث تتوفر الهوية؛ [Rust Instant](https://doc.rust-lang.org/std/time/struct.Instant.html) لقياس queue/load/generation/validation منفصلة. **OWNER DECISION D23:** إذا احتاج list معرفة locale عبر request فأضف optional field وفق D0؛ التوصية بدل تخمين لغة النص أو مشاركة cache بين اللغتين.
- **الخطوات:** أبطل العرض عند locale/evidence generation change؛ سجّل مددًا وأعدادًا محلية بلا نص السؤال أو device identifiers. ضع baseline cold/warm على جهاز Windows منخفض الذاكرة وآخر متوسط، ثم أصلح فقط المكوّن المسيطر. لا inference أثناء Care/repair critical mutation؛ queue واحدة محدودة، لا workers بعدد النقرات. احتفظ بـfallback الفوري عند disabled/loading، وبالحدود الحالية 10s insight و20s assistant دون تمديد.
- **قبول:** owner/locale cache isolation red→green؛ fake clock/backend يثبت عدم نشر stale answer؛ عشر نقرات لا تولّد عشر جلسات متزامنة؛ Windows تقرير p50/p95 مع الجهاز والنسخة وحالة cold/warm، لا رقم latency تسويقي مخترع. cancel يوقف العمل ضمن الحد الموثق backend أو تبقى حالة cancelling صادقة.
- **مخاطر/قيود:** لا نقل metrics أو auto-download؛ إزالة model من الذاكرة سياسة لاحقة إذا أثبت القياس الحاجة. H1,H3,H4,H6.

## §11 P87 — بوابات إصدار حقيقية وكلفة تشغيل محدودة

**MEASURED — الأساس:** `.github/workflows/ci.yml` يجمع Windows workspace tests وUI checks/build في job واحد، ويعدد أربعة ملفات UI unit صراحةً؛ cancellation للـCI موجود أصلًا، وكذلك caches للتنزيلات. `release.yml` يستهدف runner توقيع self-hosted وبيئة `production-signing`؛ وجود YAML لا يثبت وجود runner صالح أو شهادة. `windows-installer.yml` لديه installed probes فعلية لكن عبر workflow_dispatch. `persistence/src/lib.rs:412,498` يستعمل SQLite connection mutex وquery plans بلا owner/limit؛ الأخير أهم من استبدال قاعدة البيانات. لا أرقام حديثة لزمن CI مقاسة في هذه المراجعة.

### P87-01 — خط أساس لزمن CI ثم فصل البوابات السريعة [INFERRED، 100–220 سطر]

- **MODIFY:** `.github/workflows/ci.yml`؛ **NEW:** `AUDIT/P87-CI-BASELINE.md` في AUDIT القائم **في جلسة التنفيذ فقط**.
- **النهج:** durations من GitHub Actions jobs/steps API؛ [workflow syntax/jobs](https://docs.github.com/en/actions/writing-workflows/workflow-syntax-for-github-actions)، [dependency caching](https://docs.github.com/en/actions/using-workflows/caching-dependencies-to-speed-up-workflows).
- **الخطوات:** سجّل آخر 5 successful comparable runs من SHA/toolchain متوافقين: queue منفصل عن execution، cold/warm، تحميل ADK/compile/test/UI. انقل locale gate وUI pure unit checks إلى job سريع مستقل بالـruntime/dependencies المثبتة، واترك Windows native tests وseal/freeze إلزامية. لا duplicate cargo workspace build في job ثانٍ بلا قياس. لا توسع caches إلى أسرار/شهادات أو release binaries؛ إن أثبت compilation أنه العنصر المسيطر جرّب target cache keyed toolchain/OS/arch/features/lock hash في متابعة منفصلة، لا كحل تلقائي.
- **قبول red→green:** PR بتسريب locale يفشل fast job قبل انتظار native suite؛ required aggregate يفشل عند فشل/إلغاء أي بوابة واجبة، ولا يمر بسبب skipped job. تقرير قبل/بعد يثبت تقليل feedback latency؛ هدف مقترح ≤5min للـfast checks و≥25% خفض median إن كان فصلها مجديًا، لا وعد لزمن Windows الكلي. لا حذف اختبارات لبلوغ الرقم.
- **المخاطر/القيود:** runner minutes قد تزيد مع الفصل؛ طبق تغييرًا واحدًا وقارن. تعديل required branch checks **OWNER DECISION D24** خارج YAML؛ التوصية aggregate ثابت الاسم. H4,H5,H6.

### P87-02A — مصفوفة installed acceptance للستة أعراض [INFERRED، 180–300 سطر]

- **MODIFY:** `.github/workflows/windows-installer.yml`، `crates/ipc/examples/care_smoke.rs`، `scripts/capture-installer-screens.ps1`؛ **NEW:** `AUDIT/P87-INSTALL-ACCEPTANCE.md`.
- **النهج:** الامتداد الصغير للـinstalled probes الموجودة، لا إطار E2E جديد؛ [Windows Event Log API](https://learn.microsoft.com/en-us/windows/win32/wes/windows-event-log). UI fixtures ليست بديل real-install.
- **الخطوات:** شغّل RC على Windows11 EN ثم AR بحساب مستخدم عادي وخدمة مثبتة؛ capture نص accessibility/screenshot للـHardware/DeepScan/Repair/Care/Timeline/assistant. تأكد أن care_smoke يتحقق من ظهور run بعد reconnect/restart، وأن assessment يبلغ terminal مع unavailable provider، وno-op يشرح التغطية. destructive cases فقط في VM snapshot بموافقة صريحة ضمن الاختبار؛ لا cleanup ملفات المضيف. سجّل OS build/locale/artifact hash/test disposition لكل حالة.
- **قبول:** نسخة الأساس تفشل على الأقل اختبارات الأعراض التي ما زالت قائمة؛ بعد المهام السابقة جميع الستة تمر؛ test run يُميّز failed/skipped/not-supported ولا يعتبر الأخيرة pass. negative control raw id ونص EN في AR يجعل البوابة حمراء. لا اعتماد على ترتيب أحداث timing هش.
- **المخاطر/القيود:** hosted windows-2025 ليس إثبات Windows11 UI؛ يلزم Windows11 VM موثق. **OWNER DECISION D25:** توفير VM وأذونات اختبارات التثبيت/الإزالة/الإصلاح؛ التوصية بيئة معزولة يملكها المشروع، لا توسيع الخدمة. H1–H6.

### P87-02B — ترقية RC بعد التحقق من نفس البايتات [INFERRED، 120–220 سطر]

- **MODIFY:** `.github/workflows/release.yml`، `.github/workflows/windows-installer.yml`، `scripts/build-installer.ps1`، `scripts/verify-installer-security.ps1`.
- **النهج:** artifact provenance/hash pinning وAuthenticode؛ [WinVerifyTrust](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrust)، [protected environments](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments).
- **الخطوات:** اربط SHA وsealed source وfrozen dependency inputs وunsigned→signed hashes في receipt واضح؛ شغّل installed/security acceptance على signed RC الفعلي قبل promotion، لا rebuild بعد الاختبار. حافظ على مراجعة production-signing ورفض missing signature؛ لا تصف unsigned CI artifact كإصدار مستخدم. امنع استخدام cache غير موثوق لمادة التوقيع. اختبر update/repair/uninstall وpreservation لبيانات المالك على نفس bundle.
- **قبول:** artifact من SHA آخر أو signature فاشلة أو omitted required receipt يمنع promotion؛ signed RC ناجح وحده مؤهل. محاولة استبدال artifact بعد gate تفشل hash match. غياب signer يترك release blocked ولا يتحول unsigned fallback.
- **المخاطر/القيود:** توقيع/نشر وتنزيل أدوات CI اتصال مقصود في build environment وليس تصريح egress للمنتج. **OWNER DECISION D26:** توفير signer/channels وصلاحيات promotion؛ التوصية عدم إعلان GA حتى نجاح signed installed gate. H1,H5,H6.

### P87-03 — query خطط Care محدودة بالمالك [INFERRED، 120–220 سطر]

- **MODIFY:** `crates/persistence/src/lib.rs`، `services/maintenance-service/src/care.rs`. **NEW عند ثبوت الحاجة:** `crates/persistence/migrations/0017_p87_query_indexes.sql`؛ migration dir قائم وآخر رقم مقروء0016؛ أعد حجز الرقم من أحدث main قبل التنفيذ.
- **النهج:** owner predicate + limit في SQL، parameters مربوطة؛ [SQLite query planner](https://www.sqlite.org/queryplanner.html). احتفظ بconnection mutex؛ لا pool أو خدمة DB جديدة.
- **الخطوات:** أضف method خاصًا لاختيار Care حسب owner+eligible states+bounded count وترتيب ثابت؛ راجع callers لـplans_in_states ولا تكسر recovery الذي يحتاج نطاقًا مختلفًا. لا تغيّر SQL migration قديم، أضف index فقط إذا EXPLAIN QUERY PLAN يثبت scan مؤثرًا؛ سجل checksum عبر MIGRATIONS الحالي. اعرض أن النتائج مقيدة لو تجاوزت السقف، لا «لا مشاكل أخرى».
- **قبول:** owner B لديه آلاف الصفوف لا يغيّر نتيجة A أو تكلفة materialization لديه؛ limit+tie ordering صحيحان؛ migration يعمل fresh وعلى DB قديم وتبقى checksums القديمة كما هي. round-trip Care frozen digest unchanged. اختبار الأداء يثبت عدد rows المحمّلة لا timing عابر فقط.
- **المخاطر/القيود:** truncation يجب أن يكون ظاهرًا؛ لا حذف بيانات/retention تلقائي. H2,H3,H5,H6. تغيير retention لاحق **OWNER DECISION**؛ التوصية خارج النطاق.

### P87-04A — Timeline لا ينهار عند تراكم السجل [INFERRED، 150–250 سطر]

- **MEASURED:** `timeline-intelligence/src/ingest.rs:135` يجمع حتى2000 من كل مصدر و100 scans؛ `engine.rs:49` يرفض فوق MAX_TIMELINE_EVENTS. إضافة Care لا يجوز أن تكدّس مصدرًا آخر بلا حد إجمالي.
- **MODIFY:** `crates/timeline-intelligence/src/ingest.rs`، `crates/timeline-intelligence/tests/ingestion.rs`، `services/maintenance-service/src/timeline.rs`.
- **النهج:** merge/sort بمفتاح ثابت ثم global cap قبل builder؛ [Rust sort_by](https://doc.rust-lang.org/std/primitive.slice.html#method.sort_by). لا إعادة كتابة مخزن الأحداث.
- **الخطوات:** استعمل timestamps+source+event identity، dedup الحقيقي فقط، واقطع الأقدم بعد الدمج لا حسب أسبقية المصدر. احتفظ بالـwatermark وowner. مثّل bounded window بوضوح في response إذا يوجد حقل مناسب؛ إن لم يوجد، D0 بدل اختلاق معنى لحقل قائم.
- **قبول:** أكثر من cap موزعة على مصادر عدة ينتج أحدث cap بلا CapacityExceeded؛ الحدث الجديد من Care لا يختفي لصالح مصدر قديم؛ tie order ثابت؛ cross-owner empty؛ لا تسقط أحداثًا متمايزة تشترك بالوقت.
- **مخاطر/قيود:** memory bounded بالـper-source caps الحالية، لا streaming framework لازم. H3,H5,H6؛ wire coverage indicator **OWNER DECISION D27** إذا لزم.

### P87-04B — Pagination ثابتة ضمن snapshot [INFERRED، 180–300 سطر؛ بعد P79-01]

- **MODIFY:** `services/maintenance-service/src/timeline.rs`، `crates/contracts/proto/timeline.proto`، `apps/ui/src/features/timeline/controller.ts`، `crates/timeline-intelligence/tests/adversarial.rs`؛ اختبارات service inline.
- **النهج:** cursor opaque يحمل snapshot watermark+آخر timestamp/identity أو snapshot token bounded؛ [Protocol Buffers compatibility](https://protobuf.dev/programming-guides/proto3/#updating).
- **الخطوات:** **OWNER DECISION D28:** التوصية cursor additive مع fallback لعميل قديم، لا تغيير معنى integer cursor بصمت. ثبّت snapshot الخاص بالمالك أثناء الصفحات؛ reject expired/tampered/cross-owner cursor برسالة محلية وإعادة تحميل معلنة. لا HashMap بلا TTL/cap إذا اختير token.
- **قبول:** إدراج حدث جديد بين صفحتين لا يكرر ولا يتخطى حدثًا ضمن snapshot؛ cursor لمالك آخر يُرفض؛ صفر initial cursor يبقى صحيحًا؛ انتهاء window يطلب refresh لا empty success.
- **مخاطر/قيود:** pagination وليس أرشيفًا غير محدود؛ لا تضمين PII في cursor. H3,H4,H5,H6.

### P87-05 — قياس CPU بزمن العينة الحقيقي [INFERRED، 100–180 سطر]

- **MEASURED:** `performance-telemetry/src/windows_impl.rs:352–373` يتجاهل interval ويفصل عينتي GetSystemTimes بنحو100ms؛ `sample(...).into_snapshot(interval)` يمرّر المطلوب لا المقاس. هذا يوافق صف DBT-P49-004 المفتوح.
- **MODIFY:** `crates/performance-telemetry/src/windows_impl.rs`، `crates/performance-telemetry/src/lib.rs`، `crates/performance-telemetry/tests/native_providers.rs`.
- **النهج:** [GetSystemTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes) مع monotonic elapsed؛ لا جعل CPU API يقيس أكثر مما يقيس.
- **الخطوات:** احتفظ بـwall elapsed الفعلي للزوج الذي حسب utilization؛ لا وسم جميع CPU/disk/network counters بنافذة موحدة إذا كانت مختلفة. أضف timing لكل reading داخليًا أو وحّد collection window صراحةً دون blocking أطول؛ أي wire timing جديد D0. حالة warm-up لا تُعرض صفرًا.
- **قبول:** clock fake يطلب2s لكن زوج العينة125ms، metadata يذكر125ms؛ delayed scheduling يعطي elapsed الحقيقي؛ zero/counter-reset لا divide-by-zero أو نسبة وهمية. Windows sample يؤكد monotonic duration مع سماحية موثقة.
- **مخاطر/قيود:** jitter طبيعي، ليست microbenchmark للمستخدم. H3,H6؛ **OWNER DECISION D29** إذا احتاج عقد القياس توسيعًا.

### P87-06 — scheduler يفصل خدمة تعمل عن عملية servicing نشطة [INFERRED، 150–250 سطر]

- **MODIFY:** `crates/idle-scheduler/src/windows_state.rs`، `crates/idle-scheduler/src/runtime.rs`؛ tests inline.
- **MEASURED:** `probe_servicing_state` يجعل SERVICE_RUNNING لأي TrustedInstaller/UsoSvc/WaaSMedicSvc دليل Busy. fast sample/slow cache موجودان فعلًا؛ لا تعاد كتابتهما.
- **النهج:** [IUpdateInstaller::IsBusy](https://learn.microsoft.com/en-us/windows/win32/api/wuapi/nf-wuapi-iupdateinstaller-get_isbusy) إشارة محدودة لعمليات WUA فقط، لا برهان غياب كل CBS؛ QueryServiceStatusEx قرينة لا حكم. لا online search.
- **الخطوات:** ميّز verified busy/uncertain/idle؛ لا تقلّب running service مباشرةً إلى Idle. اجمع الإشارات المحلية الموثقة الموجودة، واجعل نقص دليل CBS/servicing Unknown محافظًا مع سبب محلي واضح. لا slow probe إذا لا عمل مستحق وفق policy، واستبق العمل قبل التنفيذ بعينة حديثة مع fast preemption كما هو. إذا لا API مدعوم يثبت الأمان فابق conservative وفسّر التأجيل بدل workaround registry غير موثق.
- **قبول:** running-idle service وحدها لا تُعرض للمستخدم «تحديث يُثبت الآن»؛ busy حقيقي يمنع mutation، unknown لا يسمح بها؛ no work=لا slow refresh جديد؛ cancel/input activity يوقف الجدولة كما سابقًا.
- **مخاطر/قيود:** تغيير سياسة إتاحة maintenance **OWNER DECISION D30**؛ التوصية لا تخفيف الحاجز قبل Windows evidence. H1,H2,H3,H5,H6.

### P87-07 — ملف مفتاح CLI بـDACL صريحة عند الإنشاء [INFERRED، 150–250 سطر]

- **MEASURED:** `apps/aetherctl/src/offline.rs` يستعمل BCrypt وcreate_new لكن Windows permissions report يصف ACL بـinherited؛ DBT-P75-005 ما زال مفتوحًا. لا تعاد معالجة RNG المغلقة.
- **MODIFY:** `apps/aetherctl/src/offline.rs`، `apps/aetherctl/tests/keys_generate_windows_rng.rs`، `apps/aetherctl/tests/keys_generate.rs`.
- **النهج:** CreateFileW(CREATE_NEW) مع SECURITY_ATTRIBUTES/DACL عند الإنشاء؛ [إنشاء security descriptor](https://learn.microsoft.com/en-us/windows/win32/secauthz/creating-a-security-descriptor-for-a-new-object-in-c--). reuse Win32 bindings وSID helpers الحالية إن توفرت.
- **الخطوات:** owner SID الحقيقي + SYSTEM/admin عند الحاجة الموثقة فقط، protected DACL تمنع inheritance الواسع، no follow/reparse substitution، fail closed إذا فشل تأمين الهدف؛ لا كتابة secret ثم تضييق ACL لاحقًا. لا طباعة seed في errors. لا تعدّل pipe ACL.
- **قبول:** parent مفتوح Everyone لا ينتج ملف مفتاح مقروءًا لحساب ثانٍ؛ existing/reparse target لا يُستبدل؛ توليد عادي يعمل؛ Unix0600 unchanged؛ فشل ACL لا يترك seed مكشوفًا.
- **مخاطر/قيود:** split-token/SID resolution يُختبر فعليًا على Windows؛ صلاحية admin ليست تصريح قراءة كل مفاتيح المستخدمين في واجهة المنتج. H5,H6؛ إضافة dependency **OWNER DECISION** إن اضطر التنفيذ، التوصية تجنبها.

### P87-08 — تمييز مالك profile في تدقيق الملفات [INFERRED، 120–220 سطر]

- **MEASURED:** `crates/security-audit/src/filesystem.rs:foreign_access` يستثني owner وSYSTEM/admin؛ elevated file ذو owner=Administrators وACE للمستخدم نفسه قد يصنّف وصوله foreign، كما يذكر DBT-P75-008.
- **MODIFY:** `crates/security-audit/src/filesystem.rs`، `crates/security-audit/src/scope.rs`، `crates/security-audit/tests/filesystem_posture.rs`.
- **النهج:** trusted OS-resolved profile owner SID الموجود في scope، مع [Windows access-control model](https://learn.microsoft.com/en-us/windows/win32/secauthz/access-control-model). لا تجاهل ACE مجهولة أو اعتبار Users آمنًا.
- **الخطوات:** مرّر SID الموثوق للprofile إلى تقييم ACL واختبره كذات المالك؛ لا تقبل SID من اسم مجلد أو wire request غير موثوق. حافظ على foreign-user/group detection وعلى unknown ACL result عند read failure.
- **قبول:** elevated own-user ACE لا warning كاذب؛ ACE لحساب آخر/Everyone/Users ما زالت warning حسب الحقوق؛ SID spoof وACL read failure لا healthy. اختبر user token وsplit admin token.
- **مخاطر/قيود:** تقليل false positive لا توسيع permissions. H3,H5,H6.

### P87-09 — إغلاق فجوات الأجهزة والتثبيت بالدليل لا بتعديل المنطق [INFERRED، جلسة تقرير وقبول، بلا ميزة جديدة]

- **MODIFY في التنفيذ فقط:** `docs/LEDGER.md` صفوف الديون ذات الصلة؛ **NEW:** `AUDIT/P87-HARDWARE-ACCEPTANCE.md`. مصادر الاختبار القائمة: `.github/workflows/windows-installer.yml`، `scripts/gate4-preconditions.ps1`، `scripts/phase16-installer-lifecycle.ps1`، `scripts/verify-installer-security.ps1`؛ لا تعدّلها إلا في مهمة إصلاح منفصلة عند إخفاق محدد.
- **النهج:** Windows11 VM/أجهزة حقيقية وnegative controls؛ [Windows Installer logging](https://learn.microsoft.com/en-us/windows/win32/msi/normal-logging). لا قراءة أدلة قديمة لإعلان نجاح جديد.
- **الخطوات:** ثبّت مصفوفة stock WindowsTemp ACL مقابل split-token وحالة parent pre-created/locked/held handle (DBT-P49-003،P74-002)، VC++ runtime absent (P41-001)، ARM64 والتوقيع وحزم recovery/media حسب الصفوف المفتوحة P55. سجّل نتيجة كل حالة والشروط التي لم تُختبر. تحقق no-egress idle باستخدام التقاط Windows مستقل مع process/service attribution، ثم explicit driver search في نافذة consent منفصلة. لا تصنّف نشاط Windows الخلفي تلقائيًا telemetry من AetherCore أو العكس.
- **قبول:** negative control malicious parent يمنع unsafe write ويظهر فشلًا مفهومًا؛ stock install لا يدّعي logs محمية بلا evidence؛ لا إغلاق صف بلا raw receipt/OS/hash؛ جهاز غير متاح=NOT RUN مع blocker محدد. warm idle/app open/model loaded لا يولّد اتصالات من المنتج؛ إلغاء consent يمنع search/download.
- **المخاطر/القيود:** **OWNER DECISION D31:** توفير الأجهزة/صور VM/شهادة/وسائط، وموافقة الاختبارات المدمرة المحددة؛ التوصية release claims محصورة بالمصفوفة المجربة. لا تغيير DACL المتعمد ولا محاولات حل debt بتخفيفه. H1–H6.
## §12 ترتيب التنفيذ وقرارات المالك ومعيار الإغلاق

**INFERRED — المسار الحرج:**

1. P77-01 ثم02 ثم03؛ P77-04A و04B جلستان. هذا حاجز لكل نص جديد من المراحل اللاحقة.
2. P78-01/02 بالتوازي المنطقي مع P79-01/02/03، لا وكلاء يعدّلون الملفات المشتركة في وقت واحد. P79-04A/B ينتظر D5 وP84-02A إذا استُعمل أي scan قد يستدعي online search. يمكن حصره cleanup-only كما توصي الخطة.
3. P78-03 عقد فقط بعد D3؛ assessment cancel في02، أما repair cancel فـP85-02/03 قبل إتاحة الزر الكامل فيP78-04. لا زر وهمي في الإصدار الوسيط.
4. P80-02A/B بعد D6 ثم producers P81/P82/P83؛ توصيل P83-05A/B و06A/B شرط إغلاق الحساسات. كل عنوان A/B يعني جلستين بمعيار قبول مستقل، لا دفعة واحدة.
5. P84-01 مستقل؛ P84-02A ثم02B ثم03/04 ثم05. لا download/install قبل D14 وضمانات03/04؛ WUA search consent منفصلة عن install consent.
6. P86-01 سدّة عاجلة يمكن تقديمها بعد P77؛ P86-02A/B بعد D22، ثم03A/B وربط السياق في04؛05 بعد D23 إذا احتاج العقد. لا يتوقف المنتج على availability النموذج.
7. P87-01 و03–08 مهام محددة قابلة للتقديم بحسب خطرها؛ P87-02A/B و09 بوابة RC بعد اكتمال نطاقه. لا انتظار كل ميزة اختيارية كي تُصلح الأعراض الستة.

**INFERRED — قرارات المالك مجمّعة (الافتراضي قبل القرار: لا تنفيذ الجزء المحجوب):**

| القرار | المطلوب تحديدًا | التوصية |
|---|---|---|
| D0,D1,D3,D5,D6,D13,D23,D27–D29 | إضافات wire/desktop للخطأ والإلغاء والتحضير والقياسات واللغة/cursor/timing | إضافات صغيرة متوافقة بعد مراجعة tags؛ لا إعادة تصميم شامل للعقد |
| D2,D14,D18 | passive offline، online driver intent، مصدر repair | offline افتراضي؛ online driver search/download بفعل صريح محدود؛ repair محلي حتى طلب منفصل |
| D4,D8,D10,D15–D17,D19–D21,D30 | consent/reboot/startup/install/protection/cancel/repair/cleanup/scheduler | لا توسيع ضمني لموافقة Care، لا bypass للحماية، لا restart تلقائي |
| D7,D9,D12 وأي dependency جديدة | vendor sensors، dump tooling/symbols، ETW | خارج الإصدار الأول؛ coverage صادقة بدل kernel driver أو تنزيل خلفي |
| D11 | active connectivity tests/فتح صفحة تقود لفحص online | لا probing إنترنت في فحص محلي؛ خطوة اختيارية موضحة |
| D22 | ضمان حقائق المساعد | قوالب facts يختارها النموذج؛ لا وعد semantic correctness للنثر الحر |
| D24–D26,D31 | required CI gates، Windows11 VM، signer، promotion والأجهزة | provisioning قبل إعلان release-ready؛ لا green من skipped tests |

**INFERRED — نطاق إصدار أول قابل للتسليم:** الأعراض الستة مغلقة على تثبيت EN/AR حقيقي، افتراضات online الصامتة مغلقة، driver success وrepair verdict صادقان، grounding مغلق، seal/freeze/install-security تمر. بعدها تُسلّم زيادة التغطية على دفعات وفق الأجهزة المدعومة. لا وعد بتغطية HWiNFO لكل sensor vendor من Windows APIs العامة وحدها؛ لا ادعاء «أفضل تجاريًا» بلا هذه المصفوفة.

**MEASURED — تحقق المسارات لهذه الخطة:** ملفات MODIFY المسماة بـapps/crates/services/scripts تحققت بأسماء المصدر ووجود الملف؛ أدوات UI في `apps/ui/tools/verify-arabic.mjs` و`apps/ui/tools/layout-sweep.mjs` موجودة. NEW المقصودة غير موجودة في لقطة المراجعة: MeasurementRows.svelte، assistant.test.ts، thermal.rs، battery.rs، network.rs، boot.rs، whea.rs، etw_capture.rs،0017_p87_query_indexes.sql، وتقارير AUDIT المقترحة. مجلدات آبائها موجودة؛ لا تنشئ أيًا منها في جلسة التخطيط. عبارة «القاموسان» تعني دائمًا `apps/ui/src/lib/i18n/catalog.en.ts` و`apps/ui/src/lib/i18n/catalog.ar.ts`. روابط API أدلة واجهات، وليست دليل تشغيل على جهاز المستخدم.

**INFERRED — ما لا يحتاج بنية جديدة:** لا microservices ولا event bus جديد ولا plugin framework ولا vector store ولا تحديث تلقائي للنموذج ولا Rust workspace split لمجرد الحجم. عنق الزجاجة المثبت من المصدر هو حدود work/queries/state ومعاني النتائج، وليس اختيار Rust/SQLite/Tauri. كل ادعاء latency أو hardware coverage أو توقيع أو عدم egress يحتاج قياسًا لاحقًا؛ لا يُغلق بتغيير نص التقرير.

## §13 Prompt جاهز لأول جلسة P77

```text
نفّذ P77-01 فقط من AUDIT/ASTRA-PLAN.md في مستودع AetherCore، بعد قراءة تعليمات المستودع الحية والتحقق من أحدث main ومن عمل P76 الجاري دون الكتابة فوق أي تغيير لوكيل آخر. لقطة الخطة كانت1581d22؛ لا تفترض أنها أحدث نسخة. لا تنفّذ P77 كاملة، ولا تضف اعتماديات أو تغيّر wire أو consent أو pipe DACL أو networking.

الهدف: حد عرض مشترك يفشل مغلقًا للنثر المملوك للمنتج ولـunknown semantic IDs في EN/AR، بدل عرض backend English أو raw IDs، مع الحفاظ على بيانات الجهاز التقنية المعزولة الاتجاه.

ملفات النطاق، من phase21-workspace/:
- apps/ui/src/lib/i18n/semantic.ts
- apps/ui/src/lib/i18n/catalog.en.ts
- apps/ui/src/lib/i18n/catalog.ar.ts
- apps/ui/src/design/primitives/LocalizedOwnedText.svelte
- apps/ui/tests/wire-values.test.ts

اقرأ الدوال المعنية فقط وتتبّع جميع callers ببحث ضيق قبل تغيير return/fallback؛ لا dump للمستودع. احتفظ بالهوية الأصلية للمنطق، وغيّر العرض فقط. لا blanket allowlist لـTechnicalText أو لأي Latin text. اسم جهاز مثل Samsung 980، version، path، والأرقام بيانات وليست نثرًا مملوكًا للمنتج.

ابدأ باختبار أحمر للقيم المجهولة، وHardware telemetry: Backend exploded، وpreflight rejected: Unknown message، ونص إنجليزي محقون داخل placeholder. يجب ألا يعود أي منها كما هو من طبقة العرض العربية أو داخل aria/title. اختبر حالات معروفة تحفظ المعنى EN/AR، ولا تجعل كل شيء Unknown لإجبار الاختبارات على المرور. localizeOwnedText وsemantic label fallbacks تعيد مفتاحًا محليًا آمنًا؛ النثر الخام لا يتحول إلى TechnicalText. لا تضف مخزن أخطاء جديدًا؛ حافظ على السجل/معرّف الارتباط الموجود إن توفر.

شغّل الاختبار قبل التنفيذ واحتفظ بسبب الفشل، ثم أصغر تغيير مشترك واختبره بعده. استعمل test command الموجود في CI من phase21-workspace، مع ملف الاختبار المحدد أولًا؛ check/i18n gate وفق package scripts الحية. rendering verification يستعمل layout fixture/sweep القائم إذا احتاج تعديل LocalizedOwnedText تحقق DOM؛ لا تنشئ browser framework جديدًا. إذا احتاج fixture تغييره، اجعلها متابعة P77-03 بدل توسعة الجلسة، وصرّح بأن DOM acceptance لم يُثبت بعد. نجاح unit وحده ليس نجاح تثبيت Windows.

التزم بـsource seal وdependency freeze وبروتوكول التوثيق المحلي؛ لا تقرأ أو تغيّر manifests يدويًا ولا تحدّث freeze. أي regeneration لختم المصدر مطلوب فقط عبر آلية المشروع المعتمدة ولملفاتك المعروفة؛ لا تضم عمل وكيل آخر. لا commit/push/deploy دون تفويض التنفيذ الخاص بالمالك. سقف الجلسة نحو300 سطر غير مولد؛ لا تبدأ مهمة أخرى.

سلّم: root cause والملفات المتغيرة، الاختبار الأحمر وسببه ثم النتيجة الخضراء الفعلية، أي acceptance بقي غير مقاس، ولا ادعاء إغلاق تعريب جميع الصفحات بهذه الجلسة وحدها. إذا أصلح main هذه الدوال فعلًا، أثبت regression coverage الناقصة فقط ولا تعاود الحل.
```
