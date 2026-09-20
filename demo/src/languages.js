// Languages shipped with the demo. `pack` is the data pack in
// `public/packs/` (built by scripts/build-packs.sh) and `rules` the matching
// rule inventory in `public/rules/`. `variant` selects variant-specific
// resources (spelling dictionary, variant rules); `size` is the gzipped pack
// size shown in the picker.
const ENGLISH = {
  sample:
    "This are a test. teh quick brown fox jumps over the lazy dog. The report was very unique.",
};
const GERMAN = {
  sample: "Das ist ein schönes Haus. Vieleicht habe ich nach Hause gegangen.",
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
const GUARANI = {
  sample:
    "Guarani ñe'ẽ ha'e peteĩ ñe'ẽ porã. Che sy oguata tape pukúpe ha aga oñe'ẽ. " +
    "Kaa mokói mitã oho óga pe ha ha oñe'ẽ. Johetũ ajepa cheve ha Guarani. " +
    "Mba'éichapa, che angirũ. Ko'ẽrõ jajotopa tape pukúpe.",
};

export const LANGUAGES = [
  { code: "en-US", pack: "en", label: "English (US)", size: "15 MB", ...ENGLISH },
  { code: "en-GB", pack: "en", label: "English (UK)", size: "15 MB", variant: "en-GB", ...ENGLISH },
  { code: "de-DE", pack: "de", label: "Deutsch (DE)", size: "22 MB", ...GERMAN },
  { code: "de-AT", pack: "de", label: "Deutsch (AT)", size: "22 MB", variant: "de-AT", ...GERMAN },
  { code: "de-CH", pack: "de", label: "Deutsch (CH)", size: "22 MB", variant: "de-CH", ...GERMAN },
  { code: "es-ES", pack: "es", label: "Español", size: "3.7 MB", ...SPANISH },
  { code: "fr", pack: "fr", label: "Français", size: "3.3 MB", ...FRENCH },
  { code: "it-IT", pack: "it", label: "Italiano", size: "1.2 MB", ...ITALIAN },
  { code: "pt-PT", pack: "pt", label: "Português (PT)", size: "6.3 MB", ...PORTUGUESE },
  { code: "pt-BR", pack: "pt", label: "Português (BR)", size: "6.3 MB", variant: "pt-BR", ...PORTUGUESE },
  { code: "nl-NL", pack: "nl", label: "Nederlands", size: "37 MB", ...DUTCH },
  { code: "ca-ES", pack: "ca", label: "Català", size: "11 MB", ...CATALAN },
  { code: "no", pack: "no", label: "Norsk bokmål", size: "3.1 MB", ...NORWEGIAN },
  { code: "nrd", pack: "nrd", label: "Nordum", size: "3.7 MB", ...NORDUM },
  { code: "gn-ES", pack: "gn", label: "Avañe'ẽ", size: "0.35 MB", ...GUARANI },
];

export const DEFAULT_LANGUAGE = "en-US";

export function findLanguage(code) {
  return LANGUAGES.find((language) => language.code === code) ?? LANGUAGES[0];
}
