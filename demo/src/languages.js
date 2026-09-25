// Languages shipped with the demo. `pack` is the data pack in
// `public/packs/` (built by scripts/build-packs.sh) and `rules` the matching
// rule inventory in `public/rules/`. `variant` selects variant-specific
// resources (spelling dictionary, variant rules). Picker sizes are shown
// from the pack manifest (main.js), not duplicated here.
const ENGLISH = {
  sample:
    "This are a test. teh quick brown fox jumps over the lazy dog. The report was very unique.",
};
const GERMAN = {
  sample: "Das ist ein schönes Haus. Vieleicht habe ich nach Hause gegangen.",
};
const SIMPLE_GERMAN = {
  sample:
    "Die Durchführung der Untersuchung war erfolgreich. Das ist ein Haus, das groß ist.",
};
const SPANISH = {
  sample: "Este son un prueba. Habían muchas personas en la calle.",
};
const FRENCH = {
  sample: "Les enfants joues dans le parc. Je suis aller au marché.",
};
const ITALIAN = {
  sample: "Qual'è il problema? Un pò di pane. Questo è un erore.",
};
const PORTUGUESE = {
  sample:
    "Este é um teste. Haviam muitas pessoas na rua. Eu gosto muito de portuguêz.",
};
const DUTCH = {
  sample: "Dit is een tesst. Hij heb gelopen en daarna heeft hij thuis geslapen.",
};
const CATALAN = {
  sample: "Aquest es un test. Hi havien moltes persones al carrer.",
};
const NORWEGIAN = {
  sample: "Dette er en tesst. Jeg har gådd hjem.",
};
const NORDUM = {
  sample:
    "Det er ein bra dag. Eg backa bilen. Eg har en datamaskin. " +
    "Mellom huset og skogen arbetar halvtreds personer. Norsk og Dansk er bra språk.",
};
const NYNORSK = {
  sample:
    "Eg veit at han kjem ikkje i dag. Jeg har også ein bil. " +
    "Han snakker om det i kirken. Dette er min bilen.",
};
const GUARANI = {
  sample:
    "Guarani ñe'ẽ ha'e peteĩ ñe'ẽ porã. Che sy oguata tape pukúpe ha aga oñe'ẽ. " +
    "Kaa mokói mitã oho óga pe ha ha oñe'ẽ. Johetũ ajepa cheve ha Guarani. " +
    "Mba'éichapa, che angirũ. Ko'ẽrõ jajotopa tape pukúpe.",
};

const GALICIAN = {
  sample: "Este son un probe. Habían moitas persoas na rua.",
};
const ROMANIAN = {
  sample: "Acesta sunt un test. Au fost multe persoane pe strada.",
};
const POLISH = {
  sample: "To jest test. Wczoraj poszłem do domu i widziałem ładny samochód.",
};
const SLOVAK = {
  sample: "Toto su test. Včera som šiel domov a videl pekný dom.",
};
const SLOVENIAN = {
  sample: "To so test. Včeraj sem šel domov in videl lep avto.",
};
const GREEK = {
  sample: "Αυτό είναι ένα τέστ. Χτές πήγα σπίτι και είδα έναν φίλο.",
};
const DANISH = {
  sample: "Dette er en tesst. Jeg har gådd hjem og spist aftensmad.",
};
const SWEDISH = {
  sample: "Det här är ett tesst. Jag har gått hem och ätit middag.",
};
const ICELANDIC = {
  sample: "Þetta er tesst. Ég elska íslensku og Ísland.",
};
const ESPERANTO = {
  sample: "Tio estas tesst. Mi amas ĉokoladon , kaj kafon.",
};
const ASTURIAN = {
  sample: "Foi a el cine cola so hermana. Voi , y depués vengo.",
};
const BRETON = {
  sample: "Klañv pe klañvoc’h. Ur ger zzqqx am eus.",
};
const TAGALOG = {
  sample: "Kumain ako ng kanin . Pinalakad ng ng abogado si Maria.",
};
const LITHUANIAN = {
  sample: "Jaroslavas pajuto kad jo draugas yra Mantas. Jaroslavas pajuto kad .",
};
const CRIMEAN_TATAR = {
  sample: "Terekniñ qarşı oturdı. Meclis toplaşuvı olıp keçti.",
};
const BELARUSIAN = {
  sample:
    "З большага, гэта быў добры дзень. Вялікая айчынная Вайна — гэта тэрмін. кампутар",
};
const RUSSIAN = {
  sample:
    "Закончилось лето. дети снова сели за школьные парты. каждя семья несчастлива.",
};
const UKRAINIAN = {
  sample:
    "Закінчилось літо. діти знову сіли за шкільні парти.  кожна сім'я щаслива.",
};
const SERBIAN = {
  sample:
    "Није шија , него врат. Почела је школа. ђаци су поново сели у клупе. То се догодило 31. новембра 2014.",
};
const ARABIC = {
  sample: "على الرغم من أنّ المسألة صعبة، كما أن استخدامه سيزداد.",
};
const PERSIAN = {
  sample: "چرا? این یک آزمایش است . ممنون از شما",
};
const KHMER = {
  sample: "នោះ\u200bហើយ\u200bនឹង\u200bនេះ។ ខ្ញុំ\u200bបាន\u200bបាន\u200bទៅ។",
};
const MALAYALAM = {
  sample: "ഞാന്‍ അവളെ ഒരു പുസ്തകം നല്‍കി.",
};
const TAMIL = {
  sample: "ஏன் உன் விழிகள் என்னைப் பார்ப்பது இல்லை?",
};
const JAPANESE = {
  sample: "名詞お見る。これは簡単なテストです。",
};
const CHINESE = {
  sample: "这是一个测试。我已经消毁了所有文件。",
};

// Picker order: alphabetical by displayed label with the groups sorted by
// their main entry's label (localeCompare/ICU collation, so endonyms sort
// naturally and non-Latin scripts follow the Latin ones: Greek, Cyrillic,
// Arabic, Indic, Khmer, then CJK). Each language's variants directly follow
// its main entry (en-GB after en-US, pt-BR after pt-PT, and the "Leichte
// Sprache" variant last within the German group); the three separate
// Norwegian-area packs sit together in label order ("Nordum" < "Norsk
// bokmål" < "Norsk nynorsk"). The default selection does NOT depend on this
// order: DEFAULT_LANGUAGE (English US) below decides what the picker shows
// first.
export const LANGUAGES = [
  { code: "ast-ES", pack: "ast", label: "Asturianu", ...ASTURIAN },
  { code: "gn-ES", pack: "gn", label: "Avañe'ẽ", ...GUARANI },
  { code: "br-FR", pack: "br", label: "Brezhoneg", ...BRETON },
  { code: "ca-ES", pack: "ca", label: "Català", ...CATALAN },
  { code: "da", pack: "da", label: "Dansk", ...DANISH },
  { code: "de-DE", pack: "de", label: "Deutsch (DE)", ...GERMAN },
  { code: "de-AT", pack: "de", label: "Deutsch (AT)", variant: "de-AT", ...GERMAN },
  { code: "de-CH", pack: "de", label: "Deutsch (CH)", variant: "de-CH", ...GERMAN },
  { code: "de-DE-x-simple-language", pack: "de", label: "Leichte Sprache", variant: "de-DE-x-simple-language", ...SIMPLE_GERMAN },
  { code: "en-US", pack: "en", label: "English (US)", ...ENGLISH },
  { code: "en-GB", pack: "en", label: "English (UK)", variant: "en-GB", ...ENGLISH },
  { code: "es-ES", pack: "es", label: "Español", ...SPANISH },
  { code: "eo", pack: "eo", label: "Esperanto", ...ESPERANTO },
  { code: "fr", pack: "fr", label: "Français", ...FRENCH },
  { code: "gl", pack: "gl", label: "Galego", ...GALICIAN },
  { code: "is-IS", pack: "is", label: "Íslenska", ...ICELANDIC },
  { code: "it-IT", pack: "it", label: "Italiano", ...ITALIAN },
  { code: "lt-LT", pack: "lt", label: "Lietuvių", ...LITHUANIAN },
  { code: "nl-NL", pack: "nl", label: "Nederlands", ...DUTCH },
  { code: "nrd", pack: "nrd", label: "Nordum", ...NORDUM },
  { code: "no", pack: "no", label: "Norsk bokmål", ...NORWEGIAN },
  { code: "nn", pack: "nn", label: "Norsk nynorsk", ...NYNORSK },
  { code: "pl", pack: "pl", label: "Polski", ...POLISH },
  { code: "pt-PT", pack: "pt", label: "Português (PT)", ...PORTUGUESE },
  { code: "pt-BR", pack: "pt", label: "Português (BR)", variant: "pt-BR", ...PORTUGUESE },
  { code: "crh-UA", pack: "crh", label: "Qırımtatar tili", ...CRIMEAN_TATAR },
  { code: "ro", pack: "ro", label: "Română", ...ROMANIAN },
  { code: "sk", pack: "sk", label: "Slovenčina", ...SLOVAK },
  { code: "sl", pack: "sl", label: "Slovenščina", ...SLOVENIAN },
  { code: "sv", pack: "sv", label: "Svenska", ...SWEDISH },
  { code: "tl-PH", pack: "tl", label: "Tagalog", ...TAGALOG },
  { code: "el", pack: "el", label: "Ελληνικά", ...GREEK },
  { code: "be-BY", pack: "be", label: "Беларуская", ...BELARUSIAN },
  { code: "ru-RU", pack: "ru", label: "Русский", ...RUSSIAN },
  { code: "sr-RS", pack: "sr", label: "Српски", ...SERBIAN },
  { code: "uk-UA", pack: "uk", label: "Українська", ...UKRAINIAN },
  { code: "ar", pack: "ar", label: "العربية", ...ARABIC },
  { code: "fa-IR", pack: "fa", label: "فارسی", ...PERSIAN },
  { code: "ta-IN", pack: "ta", label: "தமிழ்", ...TAMIL },
  { code: "ml-IN", pack: "ml", label: "മലയാളം", ...MALAYALAM },
  { code: "km-KH", pack: "km", label: "ខ្មែរ", ...KHMER },
  { code: "zh-CN", pack: "zh", label: "中文", ...CHINESE },
  { code: "ja-JP", pack: "ja", label: "日本語", ...JAPANESE },
];

export const DEFAULT_LANGUAGE = "en-US";

export function findLanguage(code) {
  // Unknown codes fall back to the default entry (English US), never to
  // array position: the alphabetically first entry is not the default.
  return (
    LANGUAGES.find((language) => language.code === code) ??
    LANGUAGES.find((language) => language.code === DEFAULT_LANGUAGE)
  );
}
