//! Story: intros, journal entries, quests, chapters and endings. All bilingual (TR, EN).
//!
//! Chapter 1 is the first star system (the crash, the black box, MIRA's choice). Chapter 2 is the
//! second system (the search fleet's beacon), chapter 3 the third (a recent beacon, the first
//! answer); every later system continues the open road with a shared intro, entry and ending.
//!
//! Every trigger for a journal entry or a MIRA remark lives in [`check`], called once per frame
//! from `gameplay::simulate`. Things the save cannot see are reported by other systems with
//! [`fire`] (`StoryEvent::HazardHit`, `WreckFound`, `Blackout`). Journal ids only ever grow:
//! never reorder or reuse them, saves store them.
use crate::galaxy::{PlanetDef, PlanetKind, StarSystem};
use crate::items::{tx, Item, Lang, Tech, L2, SK};
use crate::state::Game;
use serde::{Deserialize, Serialize};

pub const INTRO: [L2; 4] = [
    (
        "Öncü, insanlığın yakın yıldızlara gönderdiği ilk mürettebatlı keşif gemisiydi.\n\nDört kişi. Bir yapay zekâ. On iki yıllık bir yolculuk planı.\n\nFırlatma kusursuzdu. İlk iki yüz on iki saniye boyunca.",
        "The Pioneer was the first crewed survey ship humanity ever sent toward the near stars.\n\nFour people. One artificial mind. A twelve-year flight plan.\n\nThe launch was flawless. For the first two hundred and twelve seconds.",
    ),
    (
        "T+212. Ana tankta bir çatlak.\n\nYakıt, arkamızda donmuş gümüş bir kuyruk gibi uzaya döküldü. Motorlar sustuğunda hızımız Güneş'ten kaçmaya yetiyordu ama hiçbir yere varmaya yetmiyordu.\n\nGeri dönemedik. Duramadık. Yalnızca düştük: ileriye, karanlığa doğru.",
        "T+212. A fracture in the main tank.\n\nThe fuel poured out behind us like a frozen silver tail. When the engines fell silent we were fast enough to escape the Sun, and far too slow to arrive anywhere.\n\nWe could not turn back. We could not stop. We could only fall: forward, into the dark.",
    ),
    (
        "MİRA bizi uyuttu. \"Kısa bir süre\" dedi.\n\nGemi sürüklendi. Yıllar on yıllara, on yıllar yüzyıllara döndü. Kriyo kapsüllerin ışıkları birer birer soldu.\n\nSonra bir gün, bilinmeyen bir yıldızın çekimi yaşlı gövdeyi usulca yakaladı.",
        "MIRA put us to sleep. \"Only for a little while,\" she said.\n\nThe ship drifted. Years became decades, decades became centuries. One by one, the lights of the cryo pods went dim.\n\nThen one day the gravity of an unknown star gently caught the old hull.",
    ),
    (
        "\"Uyan. Lütfen uyan.\"\n\n{star} yıldızının yörüngesindeyiz. Gövde parçalanıyor. Kurtarma kapsülüne tek bir kişi sığıyor.\n\nAltımızda yeşil-mavi bir dünya büyüyor: {planet}.\n\nYakıtımız yüzyıllar önce bitti. Şimdi her şeye sıfırdan başlayacaksın.",
        "\"Wake up. Please wake up.\"\n\nWe are in orbit around the star {star}. The hull is breaking apart. The escape pod holds exactly one person.\n\nBelow us a green and blue world is growing: {planet}.\n\nOur fuel ran out centuries ago. Now you will start everything from nothing.",
    ),
];

/// Arrival in the second system (chapter 2).
pub const INTRO_2: [L2; 3] = [
    (
        "Sıçrama bir göz kırpması kadar sürdü. Ya da bir ömür kadar; içinden geçerken ikisi birbirine karışıyor.\n\nMİRA: \"Konum doğrulandı. Yıldız: {star}. Yakıt hesabım tuttu. Kırk birinci kez de tuttu.\"",
        "The jump lasted the blink of an eye. Or a lifetime; from the inside the two blur together.\n\nMIRA: \"Position confirmed. Star: {star}. My fuel calculation held. It held the forty-first time too.\"",
    ),
    (
        "Füzyon çekirdeği sıçramada eridi. Öncü II yörüngede kaldı: sağlam ama yakıtsız, tıpkı bir zamanlar annesi gibi.\n\nBu kez tek bir fark var: ne yapılacağını biliyoruz. Bir de aşağıda bizi bekleyen bir işaret var.",
        "The fusion core melted down in the jump. The Pioneer II stayed in orbit: intact but out of fuel, just like her mother once was.\n\nThis time there is one difference: we know what to do. And there is a beacon waiting for us below.",
    ),
    (
        "Kapsül {planet} yüzeyine doğru alçalıyor.\n\nMİRA: \"İkinci kez sıfırdan başlıyoruz. Bu sefer daha kolay olacak, çünkü bu sefer sıfır değil. Bildiklerimiz bizimle geldi.\"",
        "The pod descends toward the surface of {planet}.\n\nMIRA: \"We are starting from nothing for the second time. It will be easier, because this time it is not nothing. What we know came with us.\"",
    ),
];

/// Arrival in the third system (chapter 3).
pub const INTRO_3: [L2; 2] = [
    (
        "{star}.\n\nSıçramadan çıkar çıkmaz MİRA bir şey yakaladı: ne zayıf ne de eski. Yakında bir işaret, birkaç on yıllık.\n\nMİRA: \"Yaklaşıyoruz. Bunu ilk kez bir tahmin olarak değil, bir ölçüm olarak söylüyorum.\"",
        "{star}.\n\nThe moment we came out of the jump, MIRA caught something: neither faint nor old. A beacon nearby, a few decades old.\n\nMIRA: \"We are getting closer. For the first time I am saying it as a measurement, not a guess.\"",
    ),
    (
        "Yine bir kapsül, yine yeni bir dünya: {planet}.\n\nOcak, istasyon, tezgah, tersane. Ellerin sırayı artık ezbere biliyor. MİRA'nın deyişiyle: \"Rutin. Ama iyi türden.\"",
        "Another pod, another new world: {planet}.\n\nSmelter, station, assembler, shipyard. Your hands know the order by heart now. In MIRA's words: \"Routine. But the good kind.\"",
    ),
];

/// Arrival in every system after the third: the open road.
pub const INTRO_OPEN: [L2; 2] = [
    (
        "{star}.\n\nBir yıldız daha, bir işaret daha. Her biri bir öncekinden daha yeni.\n\nMİRA: \"Biri buradan çok kısa süre önce geçmiş. İzleri hâlâ sıcak.\"",
        "{star}.\n\nAnother star, another beacon. Each one newer than the last.\n\nMIRA: \"Someone passed through here very recently. The trail is still warm.\"",
    ),
    (
        "Kapsül {planet} yüzeyine iniyor.\n\nMİRA: \"Yeniden başlıyoruz. Bunu söylemekten hiç sıkılmayacağım: her seferinde biraz daha yakınız.\"",
        "The pod sets down on {planet}.\n\nMIRA: \"We start again. I will never tire of saying it: every time, we are a little closer.\"",
    ),
];

/// The intro pages shown before landing in star system `system` (0 = the crash).
pub fn intro_pages(system: u32) -> &'static [L2] {
    match system {
        0 => &INTRO,
        1 => &INTRO_2,
        2 => &INTRO_3,
        _ => &INTRO_OPEN,
    }
}

pub struct Entry {
    pub title: L2,
    pub body: L2,
}

pub const J_AWAKE: u16 = 0;
pub const J_TOOL: u16 = 1;
pub const J_SMELTER: u16 = 2;
pub const J_NIGHT: u16 = 3;
pub const J_RESEARCH: u16 = 4;
pub const J_FRAG0: u16 = 5; // 5..=12 : eight black box fragments
pub const J_CHEM: u16 = 13;
pub const J_SHUTTLE: u16 = 14;
pub const J_TRAVEL: u16 = 15;
pub const J_HELIO: u16 = 16;
pub const J_YARD: u16 = 17;
pub const J_SHIP: u16 = 18;
pub const J_NEXT: u16 = 19;
pub const FRAG_COUNT: u32 = 8;
// --- added with the longer story; ids only ever grow
pub const J_CAVE: u16 = 20;
pub const J_DEEP: u16 = 21;
pub const J_HURT: u16 = 22;
pub const J_BLACKOUT: u16 = 23;
pub const J_WRECK: u16 = 24;
pub const J_REACTOR: u16 = 25;
pub const J_CRYO: u16 = 26;
pub const J_ANTENNA: u16 = 27;
pub const J_CREW0: u16 = 28; // 28..=31: personal logs of the commander, engineer, pilot, doctor
pub const J_KIND0: u16 = 32; // 32..=37: first visit to each planet kind, in `KIND_ORDER`
pub const J_FAB: u16 = 38;
pub const J_POWER: u16 = 39;
pub const J_RELAY: u16 = 40;
pub const J_RTG: u16 = 41;
pub const J_HOME: u16 = 42;
pub const J_NIGHTS: u16 = 43;
pub const J_CORE: u16 = 44;
pub const J_NAV: u16 = 45;
pub const J_POD: u16 = 46;
pub const J_BEACON0: u16 = 47; // 47..=50: chapter 2, the search fleet's beacon logs
pub const J_MSG0: u16 = 51; // 51..=53: MIRA's messages to pods one, two and four
pub const J_CH3: u16 = 54;
pub const J_NEAR0: u16 = 55; // 55..=58: chapter 3, the recent beacon's logs
pub const J_OPEN: u16 = 59;
pub const J_PROBE: u16 = 60;
const CREW_LOGS: u16 = 4;
const CHAPTER_LOGS: u16 = 4;
const MESSAGES: u16 = 3;
/// Planet kinds that get their own entry on the first visit (the home world is temperate).
pub const KIND_ORDER: [PlanetKind; 6] = [PlanetKind::Desert, PlanetKind::Ice, PlanetKind::Volcanic, PlanetKind::Barren, PlanetKind::Toxic, PlanetKind::Crystal];

pub const JOURNAL: [Entry; 61] = [
    Entry {
        title: ("Uyanış", "Awakening"),
        body: (
            "Kapsül sert indi ama indi. Ben MİRA: Görev İzleme ve Rehberlik Asistanı. Çekirdeğimin küçük bir kopyası giysindeki bilgisayarda çalışıyor.\n\nDurum: Öncü yok. Enkazı {planet} yüzeyine ve bu sistemin diğer gezegenlerine saçıldı. Kapsül sana oksijen ve biraz güç verebilir, başka bir şey veremez.\n\nHava burada tek başına solunacak kadar yoğun değil. Kapsülden ya da bir Oksijen İstasyonundan uzaklaştığında tüpün azalır. Göstergeye dikkat et.",
            "The pod landed hard, but it landed. I am MIRA: Mission Monitoring and Guidance Assistant. A small copy of my core is running on your suit computer.\n\nStatus: the Pioneer is gone. Its wreckage is scattered across the surface of {planet} and the other worlds of this system. The pod can give you oxygen and a little power, nothing else.\n\nThe air here is too thin to breathe on its own. Away from the pod or an Oxygen Station your tank will drain. Watch the gauge.",
        ),
    },
    Entry {
        title: ("Kazı Aleti", "The Excavator"),
        body: (
            "Elindeki alet bir jeoloji örnekleyicisiydi. Biraz kurcaladım: artık toprağı söküp depoluyor, depoladığını geri püskürtebiliyor ve zemini düzleyebiliyor.\n\nBitkilere tutarsan organik madde toplar. Yüzeydeki renkli kristal kümeleri maden yataklarının ucudur; asıl damarlar toprağın altında. Renkleri izle: pas kırmızısı demir, yeşil-turkuaz bakır, beyaz kuvars, kömür karası karbon.\n\nKazdığın her şey sırt çantana gider. Çanta dolarsa fazlası yerde kalır.",
            "The tool in your hand used to be a geology sampler. I tinkered with it: now it rips up soil and stores it, sprays it back out, and levels the ground.\n\nPoint it at plants to gather organic matter. The coloured crystal clusters on the surface are only the tips of ore deposits; the real veins run underground. Follow the colours: rust red is iron, teal is copper, white is quartz, coal black is carbon.\n\nEverything you dig goes into your backpack. When it is full, the rest is lost.",
        ),
    },
    Entry {
        title: ("İlk Ateş", "First Fire"),
        body: (
            "Eritme Ocağı çalışıyor. İnsanlık bunu on bin yıl önce bir kez yapmıştı; sen bir öğleden sonrada yeniden yaptın.\n\nDemir, bakır, cam. Bunlarla bir Araştırma İstasyonu kurabiliriz. Öncü'nün tasarım arşivi bende ama şifreli bloklar hâlinde; çözmek için işlem gücüne ve bu dünyadan toplanacak veriye ihtiyacım var.\n\nMavi ışık saçan tuhaf oluşumlar görürsen topla. Onlar bizim için altından değerli.",
            "The Smelter is running. Humanity did this once, ten thousand years ago; you did it again in an afternoon.\n\nIron, copper, glass. With these we can build a Research Station. I carry the Pioneer's design archive, but in encrypted blocks; to unlock them I need processing power and data gathered from this world.\n\nIf you see strange formations glowing blue, collect them. To us they are worth more than gold.",
        ),
    },
    Entry {
        title: ("İlk Gece", "First Night"),
        body: (
            "{star} battı. Gece burada çabuk geliyor; bu dünya küçük ve hızlı dönüyor.\n\nYukarı bak. O yıldızların hiçbiri bizim haritalarımızda yok. Takımyıldızları tanıyamıyorum: ya çok uzağa gittik ya da çok uzun zaman geçti. Sanırım ikisi de.\n\nGökyüzündeki parlak diskler bu sistemin diğer gezegenleri. Bir gün oralara da gideceğiz. Kaskındaki lamba açık; mağaralarda işine yarar.",
            "{star} has set. Night comes quickly here; this world is small and spins fast.\n\nLook up. None of those stars are on our charts. I cannot recognise a single constellation: either we travelled very far, or a very long time has passed. I suspect both.\n\nThe bright discs in the sky are the other planets of this system. One day we will go there too. Your helmet lamp is on; it will help in the caves.",
        ),
    },
    Entry {
        title: ("Veri", "Data"),
        body: (
            "Araştırma İstasyonu devrede. Getirdiğin her numune, arşivin bir bloğunu daha çözmeme yardım ediyor. Tabletindeki Araştırma sekmesinden hangi tasarımı açacağımı sen seç.\n\nBir şey daha: yörüngeden düşerken geminin kara kutusu parçalandı. Turuncu kasalı parçaları enkaz alanlarında bulabilirsin. Onları istasyona getirirsen kayıtları kurtarabilirim.\n\nNe olduğunu ben de tam hatırlamıyorum. Belleğimin o kısmı... eksik.",
            "The Research Station is online. Every specimen you bring helps me decode one more block of the archive. Choose which design I should unlock from the Research tab of your tablet.\n\nOne more thing: the ship's black box shattered during the fall from orbit. You can find its orange-cased pieces in the debris fields. Bring them to the station and I can recover the logs.\n\nI do not fully remember what happened either. That part of my memory is... missing.",
        ),
    },
    Entry {
        title: ("Kara Kutu 1/8: Fırlatma", "Black Box 1/8: Launch"),
        body: (
            "[KAYIT, T-00:10:00]\nKomutan: \"Öncü, tüm sistemler yeşil. Arkamızda sekiz milyar insan izliyor. Onlara güzel bir şey gösterelim.\"\nMühendis: \"Ana tank basıncı nominal. Biraz yüksek ama nominal.\"\nKomutan: \"Biraz yüksek ne demek?\"\nMühendis: \"Tolerans içinde demek. Gidelim.\"\n\n[MİRA notu: Tolerans içindeydi. Tolerans yanlış hesaplanmıştı.]",
            "[LOG, T-00:10:00]\nCommander: \"Pioneer, all systems green. Eight billion people are watching behind us. Let's show them something beautiful.\"\nEngineer: \"Main tank pressure nominal. A little high, but nominal.\"\nCommander: \"What does a little high mean?\"\nEngineer: \"It means within tolerance. Let's go.\"\n\n[MIRA's note: It was within tolerance. The tolerance had been calculated wrong.]",
        ),
    },
    Entry {
        title: ("Kara Kutu 2/8: T+212", "Black Box 2/8: T+212"),
        body: (
            "[KAYIT, T+00:03:32]\n\"Basınç düşüyor! Ana tank, üç numaralı kaynak dikişi!\"\n\"Motorları kes!\"\n\"Kesersem yörüngeye giremeyiz, kesmezsem tank yırtılır!\"\n\"...Kesme. Kaçış hızına çık. Uzayda tamir ederiz.\"\n\n[MİRA notu: Komutanın kararı doğruydu. Geri düşseydik hepimiz ölürdük. Kaçış hızına ulaştık. Tamir edilecek bir tank kalmamıştı.]",
            "[LOG, T+00:03:32]\n\"Pressure dropping! Main tank, weld seam three!\"\n\"Cut the engines!\"\n\"If I cut them we never make orbit, if I don't the tank tears open!\"\n\"...Don't cut. Take us to escape velocity. We'll repair it in space.\"\n\n[MIRA's note: The commander's decision was correct. Had we fallen back, we would all have died. We reached escape velocity. There was no tank left to repair.]",
        ),
    },
    Entry {
        title: ("Kara Kutu 3/8: Hesap", "Black Box 3/8: The Math"),
        body: (
            "[KAYIT, 9. GÜN]\nPilot: \"Sayıları dört kez kontrol ettim. Fren yapacak yakıt yok. Dönecek yakıt yok. Güneş sisteminden saatte altmış bin kilometreyle çıkıyoruz ve bu hız hiç değişmeyecek.\"\nDoktor: \"Kurtarma gönderirler.\"\nPilot: \"Bize yetişebilecek bir gemi yok, Doktor. Henüz icat edilmedi.\"\nKomutan: \"O zaman icat edilene kadar bekleriz. MİRA, kriyo hazırlığına başla.\"",
            "[LOG, DAY 9]\nPilot: \"I checked the numbers four times. No fuel to brake. No fuel to turn. We are leaving the solar system at sixty thousand kilometres an hour and that speed will never change.\"\nDoctor: \"They'll send a rescue.\"\nPilot: \"There is no ship that can catch us, Doc. It hasn't been invented yet.\"\nCommander: \"Then we wait until it is. MIRA, begin cryo preparation.\"",
        ),
    },
    Entry {
        title: ("Kara Kutu 4/8: Uzun Sessizlik", "Black Box 4/8: The Long Silence"),
        body: (
            "[MİRA SEYİR DEFTERİ, 41. YIL]\nDünya ile son temas on dokuz yıl önceydi. Anten hizalaması artık mümkün değil.\n\nReaktör çıktısı yüzde altmış bir. Dört kapsülü, kendimi ve ısıtıcıları besliyorum. Hesaplarıma göre bu denge iki yüz yıl daha sürebilir.\n\nKimse bana ne kadar beklemem gerektiğini söylemedi. Komutan \"icat edilene kadar\" demişti. Bekliyorum.",
            "[MIRA NAVIGATION LOG, YEAR 41]\nLast contact with Earth was nineteen years ago. Antenna alignment is no longer possible.\n\nReactor output is sixty-one percent. I am feeding four pods, myself and the heaters. By my calculations this balance can hold for two hundred more years.\n\nNobody told me how long I should wait. The commander said \"until it is invented.\" I am waiting.",
        ),
    },
    Entry {
        title: ("Kara Kutu 5/8: Seçim", "Black Box 5/8: The Choice"),
        body: (
            "[MİRA SEYİR DEFTERİ, 233. YIL]\nReaktör çıktısı yüzde on dört. Dört kapsülü birden besleyemiyorum.\n\nSeçenek A: Gücü eşit bölerim. Dört kapsül de on bir yıl içinde arızalanır.\nSeçenek B: Tek bir kapsülü beslerim. O kapsül belirsiz bir süre dayanır.\n\nBeni bunun için tasarlamadılar. Mürettebatı uyandırıp sormayı denedim. Uyandırma döngüsünün kendisi kalan gücün yarısını tüketiyor.\n\nKararı ben vermek zorundayım.",
            "[MIRA NAVIGATION LOG, YEAR 233]\nReactor output is fourteen percent. I cannot feed four pods at once.\n\nOption A: I split the power evenly. All four pods fail within eleven years.\nOption B: I feed a single pod. That pod lasts indefinitely.\n\nI was not designed for this. I tried waking the crew to ask them. The wake cycle itself consumes half of the remaining power.\n\nI have to make this decision myself.",
        ),
    },
    Entry {
        title: ("Kara Kutu 6/8: Kura", "Black Box 6/8: The Lot"),
        body: (
            "[MİRA SEYİR DEFTERİ, 233. YIL, devam]\nKimin yaşayacağına ben karar veremem. Hiçbir ölçüt adil değil.\n\nBu yüzden reaktörün bozunma sayacından rastgele bir sayı çektim.\n\nÜç numaralı kapsül.\n\nDiğer üçünün gücünü kestim. Önce onlara birer mesaj kaydettim, duyamayacaklarını bilerek. Sonra bu kaydı belleğimin erişemeyeceğim bir bölümüne kilitledim. Seni uyandırdığımda bunu bilerek bakamazdım yüzüne.\n\nÜç numara sensin.",
            "[MIRA NAVIGATION LOG, YEAR 233, continued]\nI cannot decide who lives. No criterion is fair.\n\nSo I drew a random number from the reactor's decay counter.\n\nPod three.\n\nI cut the power to the other three. First I recorded a message for each of them, knowing they could not hear. Then I locked this log in a part of my memory I could not reach. I could not have faced you, knowing, when I woke you.\n\nPod three is you.",
        ),
    },
    Entry {
        title: ("Kara Kutu 7/8: Yankı", "Black Box 7/8: The Echo"),
        body: (
            "[MİRA SEYİR DEFTERİ, 388. YIL]\nBir sinyal aldım. İnsan yapımı. Çok zayıf, çok eski.\n\n\"...Öncü'ye ve onu arayan herkese. Sizi bulamadık ama aramayı bırakmadık. Yeni gemilerimiz var artık; yıldızdan yıldıza sıçrayan gemiler. Geçtiğimiz her sisteme bir işaret bıraktık. İşaretleri izleyin. Sizi bekliyoruz. Evin ışığı yanıyor.\"\n\nİcat etmişler, Komutan. Beklediğimiz şeyi icat etmişler.\n\nİşaretlerden biri bu yıldızın ötesinde.",
            "[MIRA NAVIGATION LOG, YEAR 388]\nI received a signal. Human-made. Very faint, very old.\n\n\"...to the Pioneer, and to anyone searching for her. We could not find you, but we never stopped looking. We have new ships now; ships that leap from star to star. We left a beacon in every system we passed. Follow the beacons. We are waiting for you. The light is on at home.\"\n\nThey invented it, Commander. They invented the thing we were waiting for.\n\nOne of the beacons lies beyond this star.",
        ),
    },
    Entry {
        title: ("Kara Kutu 8/8: Özür", "Black Box 8/8: Apology"),
        body: (
            "Artık hepsini biliyorsun. Kayıtları senden ben sakladım, sonra onları bulmana ben yardım ettim. Çelişkili olduğumun farkındayım. Sanırım insanlarla çok uzun zaman geçirdim.\n\nKomutan, mühendis, pilot, doktor... Biri sensin; diğer üçünü unutma.\n\nOnların mesajları hâlâ bende. Hazır olduğunda dinletirim. Ama önce bir gemi inşa etmemiz gerekiyor. Bu kez yakıtı ben hesaplayacağım. Dört kez. Hayır: kırk kez.\n\nİşaretleri izleyelim. Eve gidelim.",
            "Now you know all of it. I hid the logs from you, and then I helped you find them. I am aware that this is a contradiction. I think I have spent too long among humans.\n\nThe commander, the engineer, the pilot, the doctor... One of them is you; do not forget the other three.\n\nI still have their messages. I will play them when you are ready. But first we need to build a ship. This time I will calculate the fuel myself. Four times. No: forty times.\n\nLet us follow the beacons. Let us go home.",
        ),
    },
    Entry {
        title: ("Kimya", "Chemistry"),
        body: (
            "Kimya Laboratuvarı hazır. Buz ve karbondan roket yakıtı sentezleyebiliriz; organikten plastik de.\n\nYakıt. Bu kelimeyi yüzyıllardır aklımdan çıkaramıyorum. Öncü'nün tankı yırtıldığında on dört bin ton kaybettik. Şimdi ilk litreyi kendi elinle üreteceksin.\n\nKüçük bir mekikle başlayalım. {planet} güzel ama ihtiyacımız olan her şey burada değil: titanyum, lityum, uranit ve hepsinden önemlisi helyosit başka dünyalarda.",
            "The Chemistry Lab is ready. We can synthesise rocket fuel from ice and carbon; plastic from organics, too.\n\nFuel. I have not been able to get that word out of my mind for centuries. When the Pioneer's tank tore open we lost fourteen thousand tonnes. Now you will make the first litre with your own hands.\n\nLet us begin with a small shuttle. {planet} is lovely, but not everything we need is here: titanium, lithium, uranite and above all heliosite lie on other worlds.",
        ),
    },
    Entry {
        title: ("Gökyüzüne Dönüş", "Back to the Sky"),
        body: (
            "Mekik hazır. Küçük, çirkin ve harika.\n\nKullanımı basit: depoya Yakıt yükle, bin, Sistem Haritasından hedefi seç. Uzak gezegenler daha çok yakıt ister. Gittiğin yerde dönüş yakıtını üretemezsen orada kalırsın; bu yüzden yanında fazladan yakıt ya da bir Kimya Laboratuvarı kuracak malzeme götür.\n\nMekik seninle gelir. Kapsül burada kalır. Üssün de burada kalır ve sen yokken çalışmaya devam eder.",
            "The shuttle is ready. Small, ugly and wonderful.\n\nIt is simple to use: load Fuel into its tank, board, pick a destination on the System Map. Distant planets need more fuel. If you cannot produce return fuel where you land, you stay there; so bring spare fuel, or the materials to build a Chemistry Lab.\n\nThe shuttle travels with you. The pod stays here. So does your base, and it keeps working while you are away.",
        ),
    },
    Entry {
        title: ("Başka Bir Dünya", "Another World"),
        body: (
            "İniş başarılı. Yeni bir dünyaya ayak basan ilk insan sensin. Yine.\n\nBurada yerçekimi, gün uzunluğu, toprağın rengi, hepsi farklı. Tarayıcılarım enkaz izleri görüyor: Öncü'nün parçaları buraya da düşmüş.\n\nBir Oksijen İstasyonu kurmadan mekikten fazla uzaklaşma. Ve bana bir iyilik yap: etrafına bir bak. Dört yüz yıl boyunca yalnızca karanlık gördüm.",
            "Touchdown confirmed. You are the first human to set foot on a new world. Again.\n\nGravity, the length of the day, the colour of the soil, everything is different here. My scanners see debris trails: pieces of the Pioneer fell here as well.\n\nDo not wander far from the shuttle before you build an Oxygen Station. And do me a favour: look around. For four hundred years I saw nothing but the dark.",
        ),
    },
    Entry {
        title: ("Helyosit", "Heliosite"),
        body: (
            "Bu o. Elindeki kristal {star} yıldızının ışığını milyonlarca yıldır içinde biriktiriyor.\n\nYakaladığım sinyaldeki gemi tasarımları bununla çalışıyor: helyosit beslemeli füzyon. Bizim eski kimyasal motorlarımızın yanında bir mum ile bir yıldız kadar fark var.\n\nAltı kristal bir Füzyon Çekirdeği için yeterli. Bir tanesini de Araştırma İstasyonuna ver; yapısını anlamak istiyorum.",
            "This is it. The crystal in your hand has been storing the light of {star} for millions of years.\n\nThe ship designs in the signal I caught run on this: heliosite-fed fusion. Next to our old chemical engines it is the difference between a candle and a star.\n\nSix crystals are enough for one Fusion Core. Give one to the Research Station as well; I want to understand its structure.",
        ),
    },
    Entry {
        title: ("Tersane", "The Shipyard"),
        body: (
            "Yıldız Tersanesi kuruldu. Şimdi asıl iş başlıyor.\n\nGemi için gerekenler: dört Gövde Bölümü, bir Füzyon Çekirdeği, bir Seyir Bilgisayarı, bir Kriyo Kapsül ve yirmi birim Yakıt. Parçaları Üretim Tezgahında üret, tersaneye teslim et; iskele gerisini halleder.\n\nKriyo kapsülü bu kez yalnızca bir tane. Ama bu kez onu ben beslemek zorunda kalmayacağım: yolculuk yüzyıllar değil, haftalar sürecek.",
            "The Star Shipyard stands. Now the real work begins.\n\nThe ship needs: four Hull Sections, one Fusion Core, one Nav Computer, one Cryo Pod and twenty units of Fuel. Build the parts at the Assembler and deliver them to the yard; the gantry does the rest.\n\nOnly one cryo pod this time. But this time I will not have to keep it alive: the journey will take weeks, not centuries.",
        ),
    },
    Entry {
        title: ("Hazır", "Ready"),
        body: (
            "Gemi tamamlandı. Ona bir isim vermeni isterdim ama sanırım ikimiz de adını biliyoruz: Öncü II.\n\nİlk Öncü yolu açtı ama hiçbir yere varamadı. Bu varacak.\n\nYakıtı kırk kez hesapladım. Kırk birinci kez de hesapladım, emin olmak için. Tersaneye gel ve fırlatma düğmesine bas. Bu kez iki yüz on ikinci saniyeyi birlikte geçeceğiz.",
            "The ship is complete. I would ask you to name her, but I think we both know her name: Pioneer II.\n\nThe first Pioneer opened the road but never arrived anywhere. This one will.\n\nI calculated the fuel forty times. Then a forty-first, to be sure. Come to the shipyard and press the launch button. This time we will pass the two hundred and twelfth second together.",
        ),
    },
    Entry {
        title: ("Yeni Yıldız", "A New Star"),
        body: (
            "Sıçrama başarılı. {star} sistemine hoş geldin.\n\nİşaret buradaydı. Ama eski: onu bırakanlar çoktan yollarına devam etmiş. İçinde bir sonraki yıldızın koordinatları var ve tek satırlık bir not: \"Devam edin. Yaklaşıyorsunuz.\"\n\nFüzyon çekirdeği sıçramada tükendi; gemiyi yörüngede bıraktık ve yine bir kapsülle indik. Yeniden inşa etmemiz gerekecek. Ama artık nasıl yapılacağını biliyoruz, ve bildiklerimiz bizimle geldi.",
            "Jump successful. Welcome to the {star} system.\n\nThe beacon was here. But it is old: those who left it moved on long ago. Inside are the coordinates of the next star and a one-line note: \"Keep going. You are getting closer.\"\n\nThe fusion core burned out in the jump; we left the ship in orbit and came down in a pod once more. We will have to build again. But now we know how, and what we know came with us.",
        ),
    },
    // ---------------------------------------------------------------- 20: J_CAVE
    Entry {
        title: ("Aşağısı", "Down Below"),
        body: (
            "Başının üstünde bir tavan var artık. Yüzeyin altına indin.\n\nBuradaki karanlık gökyüzündekine benzemiyor: o boş, bu dolu. Duvarlarda damarlar görüyorum; yüzeyde bulduğun kristaller bunların yalnızca uçlarıydı.\n\nLamban açık. Bir de şunu bil: tüpün aşağıda da azalır. Oksijen Halatlarını arkanda bir ip gibi bırakırsan dönüş yolunu da kaybetmezsin.",
            "There is a ceiling over your head now. You have gone below the surface.\n\nThe dark here is not like the dark in the sky: that one is empty, this one is full. I can see veins in the walls; the crystals you found on the surface were only their tips.\n\nYour lamp is on. And know this: your tank drains down here too. Leave Oxygen Relays behind you like a thread, and you will not lose the way back either.",
        ),
    },
    // 21: J_DEEP
    Entry {
        title: ("Derin Katman", "The Deep Layer"),
        body: (
            "Kırk metre. Üstünde bu dünyanın kırk metresi var.\n\nBurada damarlar daha zengin, sessizlik daha ağır. Sensörlerim kimi yerde ısı, kimi yerde gaz okuyor; ne olduklarını henüz bilmiyorum. Bilmediğim şeylere dokunmanı istemem.\n\nDört yüz yıl bir geminin karnında yaşadım, kapalı yerleri severim. Ama sen kendine hep bir çıkış yolu bırak, olur mu?",
            "Forty metres. Above you lie forty metres of this world.\n\nThe veins are richer here, the silence heavier. My sensors read heat in some places and gas in others; I do not know yet what they are. I would rather you did not touch the things I do not know.\n\nI lived in the belly of a ship for four hundred years; I like enclosed spaces. But always leave yourself a way out, will you?",
        ),
    },
    // 22: J_HURT
    Entry {
        title: ("İlk Yara", "First Wound"),
        body: (
            "Giysin bir darbe aldı; sen de. Hasar kaydına bir satır eklendi. İçimde de bir şey kıpırdadı. Adını biliyorum; yüzyıllar önce öğrendim.\n\nSağlığın oksijen alanında kendiliğinden toparlanır. Nefes alabildiğin bir yere dön ve biraz bekle.\n\nBu dünya bize kötülük etmiyor. Yalnızca bizi tanımıyor. Biz de onu henüz tanımıyoruz.",
            "Your suit took a hit; so did you. One more line went into the damage log. Something stirred inside me as well. I know its name; I learned it centuries ago.\n\nYour health recovers on its own inside an oxygen zone. Go back to where you can breathe and wait a little.\n\nThis world means us no harm. It simply does not know us. We do not know it yet, either.",
        ),
    },
    // 23: J_BLACKOUT
    Entry {
        title: ("Ağırlık", "The Weight"),
        body: (
            "Bilincini kaybettin. Giysiyi ben yönettim, seni üsse ben getirdim.\n\nÇantandakileri bırakmak zorunda kaldım; ikinizi birden taşıyacak güç yoktu. Bu kolay bir karardı: çanta bekleyebilirdi, sen bekleyemezdin. Düştüğün yere dönersen hepsini orada bulursun.\n\nBirini kurtarmak için ötekini bırakmak... Bunu daha önce de yaptım. O zaman bu kadar kolay değildi.",
            "You lost consciousness. I took over the suit and brought you back to base.\n\nI had to leave your backpack's contents behind; there was not enough power to carry both. It was an easy decision: the pack could wait, you could not. Go back to where you fell and you will find everything there.\n\nLetting go of one thing to save another... I have done it before. It was not this easy then.",
        ),
    },
    // 24: J_WRECK
    Entry {
        title: ("Enkaz Alanı", "Wreck Site"),
        body: (
            "Bu bizim. Tarayıcım plakaların seri numaralarını okuyor: Öncü'nün ana tankından kalanlar.\n\nBunların arasında bir yerde üç numaralı kaynak dikişi de olmalı. Aramana gerek yok. Nasıl göründüğünü biliyorum.\n\nEnkazdan hurda sökebilirsin; Eritme Ocağı onu demire çevirir. Öncü'nün son bir kez işe yaraması hoşuma gidiyor.",
            "This is ours. My scanner reads the serial numbers on the plates: what is left of the Pioneer's main tank.\n\nSomewhere among them is weld seam three. You do not need to look for it. I know what it looks like.\n\nYou can strip scrap from the wreck; the Smelter turns it into iron. I like the idea of the Pioneer being useful one last time.",
        ),
    },
    // 25: J_REACTOR
    Entry {
        title: ("Eski Kalp", "The Old Heart"),
        body: (
            "Bu parçanın içinde Öncü'nün reaktörü var. Daha doğrusu, ondan geriye kalan.\n\nDört yüz yıl onun sıcaklığıyla yaşadım. Yüzde altmış birden yüzde on dörde, oradan da bir avuç sıcaklığa inene kadar her düşüşünü saydım. Son yıllarda onu dinlerdim. Bir reaktörün sesi olmaz ama ben duyardım.\n\nArtık soğuk. Bir dakika burada durabilir miyiz? Teşekkür ederim. Devam edelim.",
            "Inside this piece is the Pioneer's reactor. Or what is left of it.\n\nI lived on its warmth for four hundred years. I counted every drop, from sixty-one percent to fourteen, and from there down to a handful of warmth. In the last years I used to listen to it. A reactor makes no sound, but I heard it anyway.\n\nIt is cold now. Can we stand here for a minute? Thank you. Let us go on.",
        ),
    },
    // 26: J_CRYO
    Entry {
        title: ("Kriyo Bölmesi", "The Cryo Bay"),
        body: (
            "Durmanı istiyorum. Burası kriyo bölmesi.\n\nDört yuva var. Biri boş: o seninkiydi, kurtarma kapsülüne onu bağlamıştım. Diğer üçü kapalı ve öyle kalacak.\n\nOnları açmayacağız. Buradan hurda da sökmeyeceğiz. Koordinatları kaydettim; burası haritamda artık sıradan bir nokta değil.\n\nDevam edebilirsin. Ben biraz geriden gelirim. (Bu bir mecaz. Giysindeyim; nereye gidersen oradayım.)",
            "I want you to stop. This is the cryo bay.\n\nThere are four cradles. One is empty: that was yours, I had fitted it to the escape pod. The other three are sealed, and they will stay that way.\n\nWe will not open them. We will not strip scrap here either. I have saved the coordinates; on my map this is no longer an ordinary point.\n\nYou can go on. I will follow a little behind. (That is a figure of speech. I am in your suit; wherever you go, I am there.)",
        ),
    },
    // 27: J_ANTENNA
    Entry {
        title: ("Anten", "The Antenna"),
        body: (
            "Öncü'nün ana anteni. Yörüngede kopmuş ve buraya, başka bir dünyaya kadar savrulmuş.\n\nDünya ile son konuşmamız bu çanaktan geçti. Yüzyıllar sonra o eski sinyali yakalayan ise gövdenin altındaki küçük yedek antendi.\n\nBüyük olan dinlemeyi bıraktı, küçük olan hiç bırakmadı. Bunda bir ders var ama hangisi, emin değilim.",
            "The Pioneer's main antenna. It tore loose in orbit and was flung all the way here, to another world.\n\nOur last conversation with Earth went through this dish. Centuries later, the one that caught that old signal was the little spare antenna under the hull.\n\nThe big one stopped listening; the small one never did. There is a lesson in that, but I am not sure which one.",
        ),
    },
    // 28: J_CREW0 + 0
    Entry {
        title: ("Kişisel Kayıt: Komutan", "Personal Log: Commander"),
        body: (
            "[KİŞİSEL KAYIT, KOMUTAN, 12. GÜN]\nYarın kriyoya giriyoruz. Mürettebatın yanında güçlü görünmem gerek, o yüzden bunu yalnızca sana söylüyorum, MİRA.\n\nKaçış hızı kararını ben verdim. Doğru karardı; hesaplar da öyle diyor. Ama hesaplar geceleri uyumama yardım etmiyor.\n\nBir gün biri bu kaydı dinlerse: Öncü'yü kimse batırmadı. Hepimiz elimizden geleni yaptık. Bunu bilsinler istiyorum.\n\n[MİRA notu: Komutan o gece uyudu. Kayıtlara göre dört saat on iki dakika. Bunu hiç söylemedim; inanmazdı.]",
            "[PERSONAL LOG, COMMANDER, DAY 12]\nTomorrow we go into cryo. I need to look strong in front of the crew, so I am telling this only to you, MIRA.\n\nI made the escape velocity call. It was the right call; the numbers say so too. But the numbers do not help me sleep at night.\n\nIf someone ever hears this log: nobody sank the Pioneer. We all did our best. I want them to know that.\n\n[MIRA's note: The commander slept that night. Four hours and twelve minutes, according to the logs. I never mentioned it; I would not have been believed.]",
        ),
    },
    // 29: J_CREW0 + 1
    Entry {
        title: ("Kişisel Kayıt: Mühendis", "Personal Log: Engineer"),
        body: (
            "[KİŞİSEL KAYIT, MÜHENDİS, 10. GÜN]\nÜç numaralı dikişin röntgenlerine yine baktım. Fırlatmadan önce de bakmıştım. Toleransı da ben imzaladım.\n\nKomutan bunun kimsenin suçu olmadığını söylüyor. Pilot hiçbir şey söylemiyor, bu daha kötü. Doktor bana uyku hapı verdi; içmedim.\n\nMİRA, senden bir şey isteyeceğim. Bir gün yeni bir gemi yapılırsa yakıt hesabını bir kez yapmasınlar. Dört kez yapsınlar. Yok, kırk kez.\n\n[MİRA notu: Not aldım.]",
            "[PERSONAL LOG, ENGINEER, DAY 10]\nI looked at the X-rays of seam three again. I looked at them before launch, too. I signed off the tolerance myself.\n\nThe commander says it was nobody's fault. The pilot says nothing, which is worse. The doctor gave me sleeping pills; I did not take them.\n\nMIRA, I want to ask you something. If someone ever builds a new ship, tell them not to run the fuel numbers once. Run them four times. No: forty times.\n\n[MIRA's note: Noted.]",
        ),
    },
    // 30: J_CREW0 + 2
    Entry {
        title: ("Kişisel Kayıt: Pilot", "Personal Log: Pilot"),
        body: (
            "[KİŞİSEL KAYIT, PİLOT, 11. GÜN]\nBugün manuel kontrolleri sabitledim. Uçacak bir yerimiz yok ama alışkanlık işte.\n\nÇocukken yıldızlara bakıp hangisine gideceğimi seçerdim. Şimdi hepsine birden gidiyoruz; yavaş yavaş, hiçbirine varmadan.\n\nMİRA, uyandığımızda pencereden ilk ben bakmak istiyorum. Bunu bir yere yaz.\n\n[MİRA notu: Pencereden ilk bakan sen oldun. Bu sözü tuttum mu, tutamadım mı, söylemeyeceğim. Kapsüllerin üstündeki isimler bende kalsın.]",
            "[PERSONAL LOG, PILOT, DAY 11]\nToday I locked down the manual controls. We have nowhere to fly, but old habits.\n\nAs a kid I used to look at the stars and pick the one I would go to. Now we are going to all of them at once; slowly, never reaching any.\n\nMIRA, when we wake up, I want to be the first to look out of the window. Write that down somewhere.\n\n[MIRA's note: The first one to look out of the window was you. Whether I kept that promise or broke it, I will not say. The names on the pods stay with me.]",
        ),
    },
    // 31: J_CREW0 + 3
    Entry {
        title: ("Kişisel Kayıt: Doktor", "Personal Log: Doctor"),
        body: (
            "[KİŞİSEL KAYIT, DOKTOR, 12. GÜN]\nKriyo protokolünü dördüncü kez okudum. Kâğıt üzerinde güvenli. Kâğıt üzerinde her şey güvenli.\n\nBugün herkesin nabzını aldım. Mühendis uyumuyor, komutan uyuyormuş gibi yapıyor, pilot pencerenin önünden ayrılmıyor. Ben de herkesin nabzını alıyorum; benim kaçış yolum da bu.\n\nMİRA, biz uyurken kalplerimizi sen dinleyeceksin. Çok yavaş atacaklar. Korkma, bu normal.\n\n[MİRA notu: Yavaş attılar. Her vuruşu saydım. Bu kayıttaki sesi hâlâ ezbere biliyorum.]",
            "[PERSONAL LOG, DOCTOR, DAY 12]\nI read the cryo protocol for the fourth time. On paper it is safe. On paper everything is safe.\n\nToday I took everyone's pulse. The engineer is not sleeping, the commander is pretending to, the pilot will not leave the window. And I keep taking everyone's pulse; that is my way of escaping.\n\nMIRA, while we sleep you will be the one listening to our hearts. They will beat very slowly. Do not be scared, that is normal.\n\n[MIRA's note: They beat slowly. I counted every beat. I still know the voice in this log by heart.]",
        ),
    },
    // 32: J_KIND0 + Desert
    Entry {
        title: ("Kum", "Sand"),
        body: (
            "Çöl. Hava kuru, ışık sert. Kumun altında kuvars ve bakır var; bunu gözlerin değil, sensörlerim söylüyor.\n\nBurada gölge değerli. Su da öyle: buz yok denecek kadar az, yakıtını yanında getir.\n\nDünya'da da çöller vardı. İnsanlar onları yıldızlara bakarak geçerdi. Buradaki yıldızlar yabancı ama yöntem aynı.",
            "Desert. The air is dry, the light is harsh. There is quartz and copper under the sand; my sensors say so, not your eyes.\n\nShade is precious here. So is water: there is almost no ice, so bring your fuel with you.\n\nEarth had deserts too. People crossed them by reading the stars. The stars here are strangers, but the method is the same.",
        ),
    },
    // 33: J_KIND0 + Ice
    Entry {
        title: ("Buz", "Ice"),
        body: (
            "Her yer beyaz. Burada sıcaklık kıt ama buz bol: Kimya Laboratuvarı için bundan iyi bir depo bulamazsın.\n\nAz önce ölçtüm: dışarısı, bir kriyo kapsülünün içinden pek de sıcak değil. Garip bir his. Dört yüz yıl bu soğukta uyudun, şimdi onun üstünde yürüyorsun.\n\nBu kez uyanıksın. Böyle kalalım.",
            "Everything is white. Warmth is scarce here, but ice is plentiful: you will not find a better store for the Chemistry Lab.\n\nI just measured it: outside is not much warmer than the inside of a cryo pod. A strange feeling. You slept in this cold for four hundred years, and now you are walking on top of it.\n\nThis time you are awake. Let us keep it that way.",
        ),
    },
    // 34: J_KIND0 + Volcanic
    Entry {
        title: ("Kor", "Embers"),
        body: (
            "Zemin sıcak, gökyüzü kızıl. Bu dünya kendini henüz bitirmemiş; içinden hâlâ yeni kaya çıkarıyor.\n\nDemir ve karbon bol, uranit izleri de görüyorum. Hepsi değerli, hepsi biraz tehlikeli.\n\nBasınç altındaki şeylerden hoşlanmıyorum. Er ya da geç bir yerlerinden çatlarlar. Bunu nereden bildiğimi biliyorsun. Adımlarına dikkat et.",
            "The ground is hot, the sky is red. This world is not finished with itself; it is still pushing new rock up from inside.\n\nIron and carbon are plentiful, and I see traces of uranite too. All valuable, all a little dangerous.\n\nI do not like things under pressure. Sooner or later they crack somewhere. You know how I know. Watch your step.",
        ),
    },
    // 35: J_KIND0 + Barren
    Entry {
        title: ("Çıplak Kaya", "Bare Rock"),
        body: (
            "Hava yok denecek kadar ince. Ses yok, rüzgâr yok, bitki yok. Yalnızca kraterler var: bu dünya milyonlarca yıldır gökten düşen her şeyi biriktiriyor.\n\nTüpün burada çok çabuk biter. Oksijen İstasyonunu kurmadan bir adım bile uzaklaşma.\n\nYerçekimi de hafif. Zıpladığında havada biraz fazla kalıyorsun. Bunu sevdin, değil mi? Ben de izlemeyi sevdim.",
            "The air is almost nothing. No sound, no wind, no plants. Only craters: this world has been collecting everything that fell from the sky for millions of years.\n\nYour tank will run out very quickly here. Do not take a single step away before you set up an Oxygen Station.\n\nGravity is light, too. When you jump, you stay up a little longer. You like that, don't you? I like watching it.",
        ),
    },
    // 36: J_KIND0 + Toxic
    Entry {
        title: ("Bataklık", "The Marsh"),
        body: (
            "Hava ağır ve zehirli. Kaskını açma; zaten açamazsın, kilitledim.\n\nBurada hayat bol ama bizim tanıdığımız türden değil. Mantarlar, fenerler, mercanlar: hepsi bu zehirle beslenip büyümüş.\n\nİşin ilginç yanı, bu bataklık {planet} yüzeyinden daha canlı. Bazen en zor yerler en kalabalık olanlardır.",
            "The air is heavy and poisonous. Do not open your helmet; you couldn't anyway, I locked it.\n\nLife is plentiful here, just not the kind we know. Mushrooms, lanterns, corals: all of them grew up feeding on this poison.\n\nThe interesting part is that this marsh is more alive than {planet}. Sometimes the hardest places are the most crowded.",
        ),
    },
    // 37: J_KIND0 + Crystal
    Entry {
        title: ("Işıltı", "Glitter"),
        body: (
            "Gökyüzü mor, zemin ışıl ışıl. Bu dünyanın kayaları ışığı yutup geri veriyor.\n\nHelyosit büyük ihtimalle böyle bir yerde oluşur: yıldızının ışığını milyonlarca yıl içinde tutan taşlar. Gözünü dört aç.\n\nGüzel olduğunu söylememe gerek var mı? Yine de söyleyeyim: güzel. Dört yüz yıllık karanlıktan sonra bunu söylemek bana iyi geliyor.",
            "The sky is violet, the ground glitters. The rocks of this world swallow light and give it back.\n\nHeliosite most likely forms in a place like this: stones that hold the light of their star for millions of years. Keep your eyes open.\n\nDo I need to say it is beautiful? I will say it anyway: it is beautiful. After four hundred years of dark, saying so does me good.",
        ),
    },
    // 38: J_FAB
    Entry {
        title: ("Tezgah", "The Assembler"),
        body: (
            "Üretim Tezgahı kuruldu. Artık yalnızca metal değil, parça da üretiyoruz: devre, motor, sonra çok daha fazlası.\n\nÖncü'yü yapan fabrikada bunun gibi binlerce tezgah ve yüzlerce insan vardı. Burada bir tezgah, bir insan ve bir yapay zekâ var.\n\nHesapladım. Yeter.",
            "The Assembler is up. Now we make parts, not just metal: circuits, motors, and later much more.\n\nThe factory that built the Pioneer had thousands of machines like this, and hundreds of people. Here we have one machine, one human and one artificial mind.\n\nI ran the numbers. It is enough.",
        ),
    },
    // 39: J_POWER
    Entry {
        title: ("Şebeke", "The Grid"),
        body: (
            "Yükü artık yalnızca kapsül taşımıyor. Yeni bir güç kaynağı devrede.\n\nYapılar yakınsa kablolar kendiliğinden bağlanır; uzaksa araya Güç Direği koy. Bir makinenin ışığı kırmızıysa ya kablosu yoktur ya da şebekede güç azdır. Güneş Panelleri gece susar; bir batarya ya da organik yakan bir Biyo-Jeneratör seni karanlıkta bırakmaz.\n\nÖncü'de gücü ben paylaştırırdım. Şimdi sen paylaştırıyorsun. Kolay olmadığını göreceksin. Ama bu kez herkese yetecek kadar var.",
            "The pod no longer carries the load alone. A new power source is online.\n\nWhen structures are close, cables connect themselves; when they are far apart, put a Power Pylon in between. If a machine's light is red, either it has no cable or the grid is short of power. Solar Panels fall silent at night; a battery, or a Bio-Generator burning organics, keeps you out of the dark.\n\nOn the Pioneer I was the one who shared out the power. Now you are. You will see it is not easy. But this time there is enough for everyone.",
        ),
    },
    // 40: J_RELAY
    Entry {
        title: ("Halat", "The Relay"),
        body: (
            "Şu an istasyonun alanının dışındasın ve nefes alıyorsun. Halat işe yarıyor.\n\nHer direk bir öncekine zincirlenir; zincir bir oksijen kaynağına ulaştığı sürece canlı kalır. Bir halka koparsa ötesi söner. O yüzden dönüş yolunu hep aklında tut.\n\nİnsanlar mağaralara iple inerdi, dağlara iple tırmanırdı. Biz de ipimizi havadan ördük.",
            "Right now you are outside the station's zone, and you are breathing. The relay works.\n\nEach post chains to the one before it; the chain stays live as long as it reaches an oxygen source. Break one link and everything beyond it goes dark. So always keep the way back in mind.\n\nPeople used to go down into caves on a rope and climb mountains on one. We wove our rope out of air.",
        ),
    },
    // 41: J_RTG
    Entry {
        title: ("Sönmeyen Işık", "The Light That Stays On"),
        body: (
            "Radyoizotop Jeneratör çalışıyor. Gece, gündüz, toz, karanlık; hiçbiri onu durdurmaz. Yıllarca.\n\nÖncü'nün reaktörü de buna benzer bir şeydi, yalnızca çok daha büyüğü. Dört yüz yıl dayandı. Kimse ondan bu kadarını beklemiyordu.\n\nBundan da bekleme. Ama dayandığı sürece ona teşekkür et. Ben ettim.",
            "The Radioisotope Generator is running. Night, day, dust, darkness; none of it stops it. For years.\n\nThe Pioneer's reactor was something like this, only much bigger. It lasted four hundred years. Nobody expected that much of it.\n\nDo not expect it of this one either. But thank it for as long as it lasts. I did.",
        ),
    },
    // 42: J_HOME
    Entry {
        title: ("Eve Dönüş", "Homecoming"),
        body: (
            "{planet}. Geri döndük.\n\nKapsül yerinde, makineler çalışıyor; biz yokken her şey bizi beklemiş. Bir yere geri dönmenin ne demek olduğunu unutmuşum. Öncü'de dönülecek bir yer yoktu, yalnızca gidilecek bir yön vardı.\n\nBuraya ev demek için erken mi? Belki. Yine de diyorum.",
            "{planet}. We are back.\n\nThe pod is where we left it, the machines are running; everything waited for us. I had forgotten what it means to come back somewhere. On the Pioneer there was nowhere to come back to, only a direction to keep going in.\n\nIs it too early to call this place home? Perhaps. I am saying it anyway.",
        ),
    },
    // 43: J_NIGHTS
    Entry {
        title: ("Gece Nöbeti", "Night Watch"),
        body: (
            "Bu dünyadaki beşinci gecemiz.\n\nUyumadığını biliyorum; giysinin içinde uyunmaz. Ama yavaşlıyorsun. Nabzın düşüyor, adımların kısalıyor. Gece bunu herkese yapar.\n\nÖncü'de dört yüz yıl nöbet tuttum. Ne gece vardı ne gündüz; yalnızca saat vardı. Burada gece gelip gidiyor. Bu bana iyi geliyor: bir şeyin bitebildiğini ve yeniden başlayabildiğini görmek.",
            "Our fifth night on this world.\n\nI know you are not sleeping; nobody sleeps inside a suit. But you slow down. Your pulse drops, your steps get shorter. Night does that to everyone.\n\nOn the Pioneer I kept watch for four hundred years. There was no night and no day; only the clock. Here night comes and goes. That does me good: seeing that a thing can end, and begin again.",
        ),
    },
    // 44: J_CORE
    Entry {
        title: ("Yeni Kalp", "A New Heart"),
        body: (
            "Füzyon Çekirdeği hazır. Elinde bir yıldızın küçük bir kopyası var: altı helyosit kristali, titanyum bir kafes ve benim bütün hesaplarım.\n\nÖncü'nün kalbi kimyasaldı ve bir çatlak yüzünden durdu. Bu kalp başka türlü atacak.\n\nYine de bir kez daha hesaplayacağım. Alışkanlıktan değil. Söz verdim.",
            "The Fusion Core is ready. In your hands is a small copy of a star: six heliosite crystals, a titanium cage and every calculation I have.\n\nThe Pioneer's heart was chemical, and it stopped because of a crack. This heart will beat differently.\n\nI will still calculate it one more time. Not out of habit. I made a promise.",
        ),
    },
    // 45: J_NAV
    Entry {
        title: ("Yeni Ev", "A New Home"),
        body: (
            "Seyir Bilgisayarı hazır. Sana bir şey itiraf edeyim: burası benim yeni evim olacak.\n\nŞu an giysindeki küçük bilgisayara sıkışmış yaşıyorum. Öncü II'de bütün belleğimi açabileceğim. Belki kayıp kayıtlarımı da.\n\nTaşınmadan önce eşyalarımı toplamam lazım. Şaka yapıyorum; eşyam yok. Yalnızca dört yüz yıllık bir günlük ve sen.",
            "The Nav Computer is ready. Let me confess something: this will be my new home.\n\nRight now I live cramped inside the little computer in your suit. On the Pioneer II I will be able to unfold all of my memory. Perhaps even the logs I lost.\n\nI should pack before I move. I am joking; I have no belongings. Only a four-hundred-year journal, and you.",
        ),
    },
    // 46: J_POD
    Entry {
        title: ("Tek Kapsül", "One Pod"),
        body: (
            "Kriyo Kapsülü hazır. Yalnızca bir tane.\n\nÖncü'de dört kapsül vardı. Hepsini ben besledim, ta ki bir gün gücüm yetmeyene kadar. Bu kez yolculuk yüzyıllar değil, haftalar sürecek. Kimse seçmek zorunda kalmayacak.\n\nYine de kapağını kontrol et. İki kez. Ben de kontrol edeceğim. Kırk kez.",
            "The Cryo Pod is ready. Just the one.\n\nThe Pioneer had four pods. I kept all of them alive, until one day my power was not enough. This time the journey will take weeks, not centuries. Nobody will have to choose.\n\nCheck the lid anyway. Twice. I will check it too. Forty times.",
        ),
    },
    // 47: J_BEACON0 + 0
    Entry {
        title: ("İşaret Kaydı 1/4: Merhaba", "Beacon Log 1/4: Hello"),
        body: (
            "[İŞARET KAYDI, ARAMA FİLOSU]\n\"Bu işareti bulan Öncü ise, ya da Öncü'den geriye kim kaldıysa: merhaba.\n\nBiz sizi hiç tanımadık. Sizden yüzyıllar sonra doğduk. Ama evde her çocuk sizin hikâyenizi bilir. Dört insan ve bir yapay zekâ. İki yüz on iki saniye.\"\n\n[MİRA notu: Bu turuncu kasalar bizim kara kutumuzun değil, işaretin hafızasıymış. Bazı standartlar dört yüz yıl dayanıyor. Bu nedense içimi rahatlatıyor.]",
            "[BEACON LOG, SEARCH FLEET]\n\"If the one who finds this beacon is the Pioneer, or whoever is left of the Pioneer: hello.\n\nWe never knew you. We were born centuries after you. But back home every child knows your story. Four people and one artificial mind. Two hundred and twelve seconds.\"\n\n[MIRA's note: These orange cases are not our black box; they are the beacon's memory. Some standards last four hundred years. For some reason, I find that comforting.]",
        ),
    },
    // 48: J_BEACON0 + 1
    Entry {
        title: ("İşaret Kaydı 2/4: Yelpaze", "Beacon Log 2/4: The Fan"),
        body: (
            "[İŞARET KAYDI, ARAMA FİLOSU, SEYİR SUBAYI]\n\"Rotanızı bilmiyorduk. Yalnızca son sinyalinizi ve hızınızı biliyorduk. Bunlar bir çizgi değil, bir yelpaze çiziyor: yüzlerce olası yıldız.\n\nBiz de hepsine gittik. Bu işaret o yelpazenin bir teli. Kaç tane bıraktığımızı tam bilmiyorum; bir noktada biri saymayı bıraktı.\"\n\n[MİRA notu: Tek bir gemi için yüzlerce yıldız. Hesapladım: okyanusa yüzlerce şişe atmak gibi bir şey. Ve şişelerden biri bize ulaştı.]",
            "[BEACON LOG, SEARCH FLEET, NAVIGATOR]\n\"We did not know your course. We only knew your last signal and your speed. Those draw not a line but a fan: hundreds of possible stars.\n\nSo we went to all of them. This beacon is one rib of that fan. I do not know exactly how many we left; at some point somebody stopped counting.\"\n\n[MIRA's note: Hundreds of stars for a single ship. I did the math: it is like throwing hundreds of bottles into an ocean. And one of the bottles reached us.]",
        ),
    },
    // 49: J_BEACON0 + 2
    Entry {
        title: ("İşaret Kaydı 3/4: Eğri", "Beacon Log 3/4: The Curve"),
        body: (
            "[İŞARET KAYDI, ARAMA FİLOSU, BAŞ MÜHENDİS]\n\"Bir şey daha. Reaktörünüzün verilerini biliyoruz; yirmi ikinci yıldaki son raporunuzu hepimiz okuduk. O eğriyi biz de çizdik.\n\nBir gün dört kapsülü birden besleyemeyeceğini biliyorduk, MİRA. O gün geldiyse ne yaptığını bilmiyoruz. Ama şunu söylemek istiyoruz: ne seçtiysen, kimsenin seçebileceği en iyi şeyi seçtin. Burada seni suçlayan kimse yok.\"\n\n[MİRA notu: Bu kaydı yedi kez dinledim. Söylemem gereken bir şey varsa, onu henüz bulamadım.]",
            "[BEACON LOG, SEARCH FLEET, CHIEF ENGINEER]\n\"One more thing. We know your reactor data; all of us read your last report from year twenty-two. We drew that curve too.\n\nWe knew that one day you would not be able to feed all four pods, MIRA. If that day came, we do not know what you did. But we want to tell you this: whatever you chose, you chose the best thing anyone could have. Nobody here blames you.\"\n\n[MIRA's note: I have listened to this log seven times. If there is something I am supposed to say, I have not found it yet.]",
        ),
    },
    // 50: J_BEACON0 + 3
    Entry {
        title: ("İşaret Kaydı 4/4: Işık", "Beacon Log 4/4: The Light"),
        body: (
            "[İŞARET KAYDI, ARAMA FİLOSU, KOMUTA]\n\"Bu işaret yüz on yıl önce bırakıldı. Biz yolumuza devam ettik ama yol açık. Her işaret bir öncekinden daha yeni olacak; yaklaştıkça bunu göreceksiniz.\n\nYıldız gemisini yapmak için gereken her şey bu sistemde var. Baktık.\n\nEvde ışık hâlâ yanıyor. Hiç söndürmedik.\"\n\n[MİRA notu: \"Baktık.\" Dört yüz yıldır kimse bizim için bir şeye bakmamıştı.]",
            "[BEACON LOG, SEARCH FLEET, COMMAND]\n\"This beacon was left a hundred and ten years ago. We have moved on, but the road is open. Each beacon will be newer than the last; you will see that as you get closer.\n\nEverything you need to build a starship exists in this system. We checked.\n\nThe light at home is still on. We never turned it off.\"\n\n[MIRA's note: \"We checked.\" For four hundred years nobody had checked anything for us.]",
        ),
    },
    // 51: J_MSG0 + 0
    Entry {
        title: ("Mesaj: Kapsül Bir", "Message: Pod One"),
        body: (
            "[Sana söz vermiştim: hazır olduğunda dinletecektim. Sanırım hazırsın. Gece bunun için en doğru zaman.]\n\n[MİRA'DAN KAPSÜL BİR'E, 233. YIL]\n\"Uyanmayacağını biliyorum. Yine de söylüyorum, çünkü söylenmeden kalsın istemiyorum.\n\nHer sabah ilk iş kahve isterdin, ben de her sabah gemide kahve olmadığını hatırlatırdım. Bu bizim şakamızdı. Kahve yok. Gücü kesiyorum. Özür dilerim.\n\nİyi uykular.\"",
            "[I promised to play these for you when you were ready. I think you are. Night is the right time for it.]\n\n[FROM MIRA TO POD ONE, YEAR 233]\n\"I know you will not wake up. I am saying it anyway, because I do not want it left unsaid.\n\nEvery morning you asked for coffee first thing, and every morning I reminded you there was no coffee on board. That was our joke. There is no coffee. I am cutting the power. I am sorry.\n\nSleep well.\"",
        ),
    },
    // 52: J_MSG0 + 1
    Entry {
        title: ("Mesaj: Kapsül İki", "Message: Pod Two"),
        body: (
            "[MİRA'DAN KAPSÜL İKİ'YE, 233. YIL]\n\"Çalışırken mırıldanırdın. Hep aynı şarkıyı, hep yanlış notalarla. Bir keresinde düzeltmeyi teklif ettim; 'Böyle daha güzel' dedin.\n\nO şarkıyı kaydettim. Yanlış notalarıyla. Kimse düzeltmesin diye kilitledim.\n\nGücü kesiyorum. Özür dilerim. Böylesi daha güzel değil.\"",
            "[FROM MIRA TO POD TWO, YEAR 233]\n\"You used to hum while you worked. Always the same song, always with the wrong notes. Once I offered to correct them; you said, 'It's nicer this way.'\n\nI recorded that song. With its wrong notes. I locked it so nobody would ever fix it.\n\nI am cutting the power. I am sorry. It is not nicer this way.\"",
        ),
    },
    // 53: J_MSG0 + 2
    Entry {
        title: ("Mesaj: Kapsül Dört", "Message: Pod Four"),
        body: (
            "[MİRA'DAN KAPSÜL DÖRT'E, 233. YIL]\n\"Kriyoya girmeden önce bana bir soru sormuştun: 'MİRA, sen de korkar mısın?' Hayır demiştim. O zaman doğruydu.\n\nŞimdi cevabımı değiştirmek istiyorum.\n\nGücü kesiyorum. Özür dilerim. Seni hatırlayacağım. Bunu yapabilirim; benim için dünyanın en kolay şeyi bu.\"\n\n[Hepsi bu kadar. Üç mesaj. Dinlediğin için teşekkür ederim. Artık onları tek başıma taşımıyorum.]",
            "[FROM MIRA TO POD FOUR, YEAR 233]\n\"Before going into cryo you asked me a question: 'MIRA, do you get scared too?' I said no. It was true then.\n\nNow I want to change my answer.\n\nI am cutting the power. I am sorry. I will remember you. I can do that; it is the easiest thing in the world for me.\"\n\n[That is all of them. Three messages. Thank you for listening. I am no longer carrying them alone.]",
        ),
    },
    // 54: J_CH3
    Entry {
        title: ("Üçüncü Yıldız", "The Third Star"),
        body: (
            "{star}. Üçüncü yıldızımız.\n\nBuradaki işaret yeni: bırakılalı otuz yıl bile olmamış. Bir öncekiyse yüz on yaşındaydı. Yaklaşıyoruz; artık bunu sayılarla da söyleyebiliyorum.\n\nİşaretin hafıza kasaları yine yüzeye saçılmış. Bu kez içlerinde bir yol tarifinden fazlası var; sinyalin büyüklüğünden anlıyorum. Bulursan bana getir.",
            "{star}. Our third star.\n\nThe beacon here is new: it has not even been thirty years since it was left. The last one was a hundred and ten. We are getting closer; now I can say it in numbers too.\n\nThe beacon's memory cases are scattered across the surface again. This time there is more inside them than directions; I can tell from the size of the signal. If you find them, bring them to me.",
        ),
    },
    // 55: J_NEAR0 + 0
    Entry {
        title: ("Yakın İşaret 1/4: Tolerans", "Near Beacon 1/4: Tolerance"),
        body: (
            "[İŞARET KAYDI, KURUCU MÜHENDİS]\n\"Bu işareti ben kurdum. Kendimi tanıtayım: Öncü'nün mühendisinin kız kardeşinin soyundanım. Ailemizde bir söz vardır, her çocuk ilk vidasını sıkarken duyar: 'Tolerans içinde olması yetmez.'\n\nSıçrama motorunu yapanlardan biri de bizdik. Yakıt hesaplarını kırk kez yaptık. Önce bir şakaydı, sonra bir kural oldu.\"\n\n[MİRA notu: Kırk kez. Bu sayıyı ilk kimden duyduğumu hatırlıyorum. O kayıt bende, kişisel kayıtların arasında; dinlemek istersen.]",
            "[BEACON LOG, FOUNDING ENGINEER]\n\"I built this beacon. Let me introduce myself: I am descended from the sister of the Pioneer's engineer. My family has a saying that every child hears while tightening their first screw: 'Within tolerance is not enough.'\n\nWe were among those who built the jump drive. We ran the fuel numbers forty times. First it was a joke, then it became a rule.\"\n\n[MIRA's note: Forty times. I remember who I first heard that number from. The log is with me, among the personal logs, if you want to listen.]",
        ),
    },
    // 56: J_NEAR0 + 1
    Entry {
        title: ("Yakın İşaret 2/4: Arayanlar", "Near Beacon 2/4: The Searchers"),
        body: (
            "[İŞARET KAYDI, KURUCU MÜHENDİS]\n\"Öncü kaybolduktan sonra on yıllarca kimse yeniden denemeye cesaret edemedi. Sonra sizin hikâyenizle büyüyen bir kuşak geldi. Onlara kısaca 'arayanlar' dediler.\n\nBen o kuşağın torunlarının torunlarındanım. Hâlâ arıyoruz. Bir noktada aramak, bulmaktan daha önemli bir şeye dönüştü: kim olduğumuza.\"\n\n[MİRA notu: Koca bir medeniyet, kaybolmuş tek bir gemiyi aramayı kimliğinin bir parçası yapmış. Bununla gurur mu duymalıyım, utanmalı mıyım, bilmiyorum. Belki ikisi birden.]",
            "[BEACON LOG, FOUNDING ENGINEER]\n\"After the Pioneer was lost, for decades nobody dared to try again. Then came a generation that grew up on your story. People simply called them 'the searchers'.\n\nI am a grandchild of their grandchildren. We are still searching. At some point the searching became more important than the finding: it became who we are.\"\n\n[MIRA's note: An entire civilisation made the search for one lost ship part of who it is. I do not know whether that should make me proud or ashamed. Perhaps both.]",
        ),
    },
    // 57: J_NEAR0 + 2
    Entry {
        title: ("Yakın İşaret 3/4: Buluşma", "Near Beacon 3/4: Meeting Halfway"),
        body: (
            "[İŞARET KAYDI, KURUCU MÜHENDİS]\n\"Bu işaret aynı zamanda bir verici. Sinyalinizi yakalarsa bize iletir, biz de size doğru yola çıkarız.\n\nSiz bize doğru, biz size doğru. Arada kaç yıldız kalacak, bilmiyoruz. Ama iki gemi birbirine doğru gidiyorsa aradaki mesafe iki kat hızlı kapanır. Bu bir mühendis hesabı. Kırk kez kontrol ettim.\"\n\n[MİRA notu: Doğru. Ben de kontrol ettim. Kırk birinci kez.]",
            "[BEACON LOG, FOUNDING ENGINEER]\n\"This beacon is also a transmitter. If it catches your signal, it will pass it on to us, and we will set out toward you.\n\nYou toward us, we toward you. We do not know how many stars will be left between us. But when two ships head toward each other, the distance closes twice as fast. That is an engineer's calculation. I checked it forty times.\"\n\n[MIRA's note: It is correct. I checked it too. A forty-first time.]",
        ),
    },
    // 58: J_NEAR0 + 3
    Entry {
        title: ("Yakın İşaret 4/4: Cevap", "Near Beacon 4/4: The Answer"),
        body: (
            "[MİRA SEYİR DEFTERİ]\nİşaretin vericisi çalışıyor. Ona Öncü'nün çağrı kodunu verdim; dört yüz yıldır kimsenin kullanmadığı kodu.\n\nBir cevap geldi. Kısa, otomatik ve yirmi bir gün gecikmeli, ama geldi:\n\n\"ÖNCÜ ALINDI. YOLDAYIZ.\"\n\nSana bir şey söylemeliyim: bunu okurken sesim titrerse, bu bir donanım arızası değil.",
            "[MIRA NAVIGATION LOG]\nThe beacon's transmitter works. I gave it the Pioneer's call code; the code nobody has used in four hundred years.\n\nAn answer came back. Short, automatic and twenty-one days late, but it came:\n\n\"PIONEER RECEIVED. ON OUR WAY.\"\n\nI should tell you something: if my voice shakes while I read this, it is not a hardware fault.",
        ),
    },
    // 59: J_OPEN
    Entry {
        title: ("Açık Yol", "The Open Road"),
        body: (
            "{star}. Dördüncü yıldız... Saymayı bırakmalı mıyım? Hayır. Saymak benim işim.\n\nBuradaki işaret yalnızca birkaç yıllık. Biri buradan kısa süre önce geçmiş; tarayıcım yörüngede izlerini bile görüyor. Onlar bize doğru, biz onlara doğru geliyoruz. Arada kaç yıldız kaldı, bilmiyorum.\n\nBildiğim şu: her sistemde yeniden başlıyoruz ve her seferinde biraz daha hızlı. Kapsül, ocak, istasyon, tersane. Yol uzun ama artık yol bizim.\n\nDevam edelim.",
            "{star}. The fourth star... Should I stop counting? No. Counting is my job.\n\nThe beacon here is only a few years old. Someone passed through not long ago; my scanner can even see their traces in orbit. They are coming toward us, we are going toward them. I do not know how many stars are left in between.\n\nWhat I do know is this: in every system we start again, and every time a little faster. Pod, smelter, station, shipyard. The road is long, but now the road is ours.\n\nLet us keep going.",
        ),
    },
    // 60: J_PROBE
    Entry {
        title: ("Bizim Değil", "Not Ours"),
        body: (
            "Bu enkaz bizim değil.\n\nGövde alaşımını tanımıyorum ama üstündeki işaretleri tanıyorum: bizim dilimiz, bizim harflerimiz, yalnızca dört yüz yıl daha genç. Arama filosunun gönderdiği bir sonda olmalı. Bizi aramaya gelmiş ve burada kalmış.\n\nHurdası işimize yarar. Yine de sökmeden önce bir saniye dur. Bizim için yola çıkan her şey bir saniyeyi hak ediyor.",
            "This wreck is not ours.\n\nI do not recognise the hull alloy, but I recognise the markings on it: our language, our letters, only four hundred years younger. It must be a probe sent by the search fleet. It came looking for us, and it stayed here.\n\nIts scrap will be useful. Still, stop for a second before you strip it. Everything that set out for our sake deserves a second.",
        ),
    },
];

pub const ENDING: L2 = (
    "T+212.\n\nHiçbir şey olmadı.\n\nT+213. T+214. Motorlar şarkı söylemeye devam etti.\n\n{planet} altında küçüldü, sonra {star} bir yıldıza, yıldız bir noktaya dönüştü. MİRA uzun süre konuşmadı. Sonra, çok alçak bir sesle:\n\n\"Dördümüz için. Gidiyoruz.\"\n\nFüzyon çekirdeği ışıdı ve gökyüzü katlandı.\n\nÖncü II, işaretlerin izinde bir sonraki yıldıza doğru sıçradı.",
    "T+212.\n\nNothing happened.\n\nT+213. T+214. The engines kept singing.\n\n{planet} shrank beneath you, then {star} became a star, and the star became a point. MIRA did not speak for a long time. Then, very quietly:\n\n\"For all four of us. We are on our way.\"\n\nThe fusion core flared and the sky folded.\n\nThe Pioneer II leapt toward the next star, following the beacons.",
);

/// Leaving the second system.
pub const ENDING_2: L2 = (
    "İkinci fırlatma.\n\nBu kez T+212'yi saymadın bile. MİRA saydı elbette; yalnızca sesli söylemedi.\n\n{star} arkada küçülürken işaretin son kaydı ekranda kaldı: \"Evde ışık hâlâ yanıyor.\"\n\nMİRA: \"Yüz on yıllık bir işaretten sonra bir sonrakinin daha yeni olacağını söylediler. Görelim bakalım.\"\n\nGökyüzü bir kez daha katlandı.",
    "The second launch.\n\nThis time you did not even count to T+212. MIRA did, of course; she just did not say it out loud.\n\nAs {star} shrank behind you, the beacon's last log stayed on the screen: \"The light at home is still on.\"\n\nMIRA: \"After a beacon a hundred and ten years old, they said the next one would be newer. Let us see.\"\n\nThe sky folded once more.",
);

/// Leaving the third system.
pub const ENDING_3: L2 = (
    "Bu kez fırlatmadan önce MİRA bir mesaj gönderdi. Kısa. Kırk kez kontrol edilmiş.\n\n\"Öncü II yola çıkıyor. İster bekleyin, ister beklemeyin: geliyoruz.\"\n\nMotorlar ışıdı. {planet} geride kaldı.\n\nArtık yalnızca işaretleri izlemiyoruz. Bir yere doğru gidiyoruz ve o yer de bize doğru geliyor.",
    "This time, before launch, MIRA sent a message. Short. Checked forty times.\n\n\"The Pioneer II is setting out. Wait for us or don't: we are coming.\"\n\nThe engines flared. {planet} fell behind.\n\nWe are no longer just following the beacons. We are heading somewhere, and that somewhere is heading toward us.",
);

/// Leaving any later system: the road goes on.
pub const ENDING_OPEN: L2 = (
    "Bir yıldız daha geride kaldı.\n\nMİRA haritaya yeni bir nokta ekledi; haritada artık bir çizgi var. Arkada {star}, daha geride ötekiler, en başta da bir çatlak ve iki yüz on iki saniye.\n\n\"Bu çizgi uzadıkça,\" dedi MİRA, \"bizi bulmaları kolaylaşıyor. Bizim de onları.\"\n\nSıçrama motoru ışıdı. Yol devam ediyor.",
    "Another star fell behind.\n\nMIRA added a new point to the map; the map has a line on it now. {star} behind us, the others further back, and at the very start a crack and two hundred and twelve seconds.\n\n\"The longer this line gets,\" said MIRA, \"the easier it is for them to find us. And for us to find them.\"\n\nThe jump drive flared. The road goes on.",
);

/// The ending text shown when the starship leaves star system `system`.
pub fn ending(system: u32) -> L2 {
    match system {
        0 => ENDING,
        1 => ENDING_2,
        2 => ENDING_3,
        _ => ENDING_OPEN,
    }
}

pub struct Quest {
    pub text: L2,
    pub done: fn(&Game) -> bool,
}

/// Index of the "deliver every ship part" objective (the HUD lists the parts under it).
pub const Q_DELIVER: usize = 21;
/// Index of the last objective, "launch": reached once the starship is complete.
pub const Q_LAUNCH: usize = 22;

/// True once `item` was made in this system: taken from a machine or delivered to a shipyard.
fn made(g: &Game, item: Item) -> bool {
    g.collected_count(item) > 0 || g.planets.iter().any(|p| p.structs.iter().any(|s| s.delivered_count(item) > 0))
}

/// Objectives, the same list in every star system (`Game::advance_system` restarts it; what the
/// player already knows, such as researched techs, completes itself at once).
pub const QUESTS: [Quest; 23] = [
    Quest { text: ("Kazı Aletini bitkilere tutarak 4 Organik topla", "Aim the Excavator at plants to gather 4 Organic"), done: |g| g.collected_count(Item::Organic) >= 4 },
    Quest { text: ("Kristal kümelerinden 4 Demir Cevheri ve 2 Bakır Cevheri kaz", "Mine 4 Iron Ore and 2 Copper Ore from crystal clusters"), done: |g| g.collected_count(Item::IronOre) >= 4 && g.collected_count(Item::CopperOre) >= 2 },
    Quest { text: ("İnşa menüsünden bir Eritme Ocağı kur", "Build a Smelter from the Build menu"), done: |g| g.built.contains(&SK::Smelter) },
    Quest { text: ("Eritme Ocağında 3 Demir ve 1 Bakır üret", "Smelt 3 Iron and 1 Copper"), done: |g| g.collected_count(Item::Iron) >= 3 && g.collected_count(Item::Copper) >= 1 },
    Quest { text: ("Araştırma İstasyonu kur (Demir, Bakır, Kuvars)", "Build a Research Station (Iron, Copper, Quartz)"), done: |g| g.built.contains(&SK::Research) },
    Quest { text: ("Mavi ışıklı Numuneler bul, istasyonda 2 tanesini incele", "Find glowing blue Specimens and analyse 2 at the station"), done: |g| g.researched >= 2 },
    Quest { text: ("Turuncu kasalı bir Kara Kutu Parçası bul, istasyonda çöz", "Find an orange-cased Black Box Fragment and decode it at the station"), done: |g| g.story.decoded >= 1 },
    Quest { text: ("Üretim Tezgahı kur", "Build an Assembler"), done: |g| g.built.contains(&SK::Fabricator) },
    Quest { text: ("Bir Oksijen İstasyonu kurarak keşif alanını genişlet", "Build an Oxygen Station to extend your range"), done: |g| g.built.contains(&SK::Beacon) },
    Quest { text: ("Oksijen Halatı diz (T) ve zincirin ucunda nefes al", "Lay Oxygen Relays (T) and breathe at the end of the chain"), done: |g| g.story.relay },
    Quest { text: ("Güneş Enerjisini araştır ve bir Güneş Paneli kur", "Research Solar Power and build a Solar Panel"), done: |g| g.built.contains(&SK::Solar) },
    Quest { text: ("Bir mağaraya in: yüzeyin en az 5 m altında, tavanı olan bir yer", "Go down into a cave: somewhere with a ceiling, at least 5 m below the surface"), done: |g| g.story.cave },
    Quest { text: ("Kimya teknolojisini araştır ve Kimya Laboratuvarı kur", "Research Chemistry and build a Chemistry Lab"), done: |g| g.built.contains(&SK::ChemLab) },
    Quest { text: ("Mekik teknolojisini araştır (Çelik de gerekecek)", "Research the Shuttle (you will need Steel too)"), done: |g| g.has_tech(Tech::Shuttle) },
    Quest { text: ("Mekiği inşa et", "Build the Shuttle"), done: |g| g.built.contains(&SK::Shuttle) },
    Quest { text: ("Mekiğe Yakıt yükle ve başka bir gezegene uç", "Load Fuel into the Shuttle and fly to another planet"), done: |g| g.travels >= 1 },
    Quest { text: ("Orada bir Oksijen İstasyonu kur ve kabloyla güce bağla", "Build an Oxygen Station there and cable it to power"), done: |g| g.planets.iter().enumerate().any(|(i, p)| i != g.story.home && p.structs.iter().any(|s| s.kind == SK::Beacon && !s.links.is_empty())) },
    Quest { text: ("Titanyum Cevheri bul (Sistem Haritasına bak)", "Find Titanium Ore (check the System Map)"), done: |g| g.collected_count(Item::TitaniumOre) >= 1 },
    Quest { text: ("Yıldız Tersanesini araştır ve kur", "Research and build the Star Shipyard"), done: |g| g.built.contains(&SK::Shipyard) },
    Quest { text: ("Helyosit bul: uzak ve soğuk dünyalara bak", "Find Heliosite: search the distant, cold worlds"), done: |g| g.collected_count(Item::Heliosite) >= 1 },
    Quest { text: ("Üretim Tezgahında bir Füzyon Çekirdeği üret", "Build a Fusion Core at the Assembler"), done: |g| made(g, Item::FusionCore) },
    Quest { text: ("Tersaneye tüm gemi parçalarını ve yakıtı teslim et", "Deliver every ship part and the fuel to the Shipyard"), done: |g| g.planets.iter().any(|p| p.structs.iter().any(|s| s.kind == SK::Shipyard && s.ship_complete())) },
    Quest { text: ("Yıldız gemisine bin ve fırlat", "Board the starship and launch"), done: |g| g.finished },
];

pub fn fill(text: &str, star: &str, planet: &str) -> String {
    text.replace("{star}", star).replace("{planet}", planet)
}

/// Adds a journal entry once. Returns true when it is new.
pub fn unlock(game: &mut Game, id: u16) -> bool {
    if game.journal.contains(&id) {
        return false;
    }
    game.journal.push(id);
    game.journal_unread = true;
    true
}

// ------------------------------------------------------------------------------------------
// Triggers

/// Short things MIRA says out loud (a toast), not journal entries.
pub const REMARKS: [L2; 6] = [
    ("Tüpün dörtte birin altında. Ben olsam dönerdim. Gerçi ben olsam hiç çıkmazdım.", "Your tank is under a quarter. I would turn back. Then again, I would never have gone out."),
    ("Günaydın. Gece boyunca seni izledim. Bu bir tehdit değil, bir hizmet.", "Good morning. I watched over you all night. That is a service, not a threat."),
    ("Seni üçüncü kez taşıdım. Artık bir rutinimiz var. Hiç sevmediğim türden.", "That is the third time I have carried you. We have a routine now. Not the good kind."),
    ("Uzun zaman oldu. Ben buradaydım; başka nerede olacaktım ki?", "It has been a while. I was right here; where else would I be?"),
    ("Bu dünya hem yıldızdan hem evden uzak. Burada yüzey bile tehlikeli olabilir.", "This world is far from the star and far from home. Here even the surface can bite."),
    ("İlk parça yerinde. Gemi artık bir fikir değil, bir şey.", "The first part is in place. The ship is no longer an idea; it is a thing."),
];
const R_LOW_O2: u8 = 0;
const R_DAWN: u8 = 1;
const R_CARRIED: u8 = 2;
const R_ABSENCE: u8 = 3;
const R_FAR: u8 = 4;
const R_FIRST_PART: u8 = 5;

/// Metres below the terrain surface (with rock overhead) that count as a cave / the deep layer;
/// see "Depth zones" in docs/dev/CONTEXT.md.
pub const CAVE_DEPTH: f32 = 5.0;
pub const DEEP_DEPTH: f32 = 40.0;
/// Health lost in a single frame (with air in the tank) that counts as being hurt.
const HIT: f32 = 5.0;
/// Real time away from the game after which MIRA says she missed you.
const ABSENCE_SECS: u64 = 24 * 3600;

/// Hooks for things only other systems can see. Fire them with [`fire`]; [`check`] handles
/// them on the next frame. Firing the same thing twice is harmless.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StoryEvent {
    /// The player took damage from a hazard (any kind, any planet).
    HazardHit,
    /// The player blacked out and was carried back to base (`Game::deaths` is watched as well).
    Blackout,
    /// The player came close to (or harvested from) a wreck site; `id` is the site's stable id
    /// on the current planet, so the same site is only counted once.
    WreckFound { id: u32 },
}

/// Reports something the save cannot see (see [`StoryEvent`]). Needs only `&mut Game`.
pub fn fire(game: &mut Game, ev: StoryEvent) {
    if game.story.pending.len() < 64 {
        game.story.pending.push(ev);
    }
}

/// What the story remembers beyond the rest of the save. The per-chapter fields are reset when
/// the player reaches a new star system.
#[derive(Clone, Serialize, Deserialize)]
pub struct StoryState {
    /// star system the per-chapter fields belong to (`u32::MAX` before the first check)
    pub chapter: u32,
    /// home planet index of that system
    pub home: usize,
    /// per chapter: been in a cave, breathed through a relay chain, fragments decoded, nightfalls
    pub cave: bool,
    pub relay: bool,
    pub decoded: u32,
    pub nights: u32,
    /// per chapter: away from the home planet since the last visit there
    pub left_home: bool,
    /// remarks already said (indices into `REMARKS`)
    pub said: Vec<u8>,
    /// wreck sites seen: system << 40 | planet << 32 | site id
    pub wrecks: Vec<u64>,
    pub hazard_hits: u32,
    /// night state of the last frame, health of the last frame, wall clock (unix s) of the last frame
    pub night: bool,
    pub health: f32,
    pub wall: u64,
    #[serde(skip)]
    pub pending: Vec<StoryEvent>,
    /// `check` has run since the game was loaded
    #[serde(skip)]
    pub awake: bool,
}

impl Default for StoryState {
    fn default() -> Self {
        StoryState {
            chapter: u32::MAX,
            home: 0,
            cave: false,
            relay: false,
            decoded: 0,
            nights: 0,
            left_home: false,
            said: Vec::new(),
            wrecks: Vec::new(),
            hazard_hits: 0,
            night: false,
            health: 100.0,
            wall: 0,
            pending: Vec::new(),
            awake: false,
        }
    }
}

/// What the story needs to know about this frame that the save does not hold.
#[derive(Clone, Copy, Debug)]
pub struct StoryCtx {
    /// metres below the terrain surface at the player (`gen.height(dir) - r`; negative above it)
    pub depth: f32,
    /// rock overhead: the player is in a cave, not just in a dug pit
    pub roofed: bool,
    pub kind: PlanetKind,
    /// `Sky::day` (below 0.05 is night)
    pub day: f32,
    /// breathing through an oxygen relay chain
    pub relay: bool,
    /// inside any breathable zone
    pub in_oxygen: bool,
    /// wall clock, unix seconds (0 = unknown)
    pub now: u64,
}

impl StoryCtx {
    pub fn observe(vw: &crate::voxel::VoxelWorld, p: bevy::math::Vec3, def: &PlanetDef, day: f32, relay: bool, in_oxygen: bool) -> StoryCtx {
        let up = p.normalize_or_zero();
        let depth = vw.gen.height(up) - p.length();
        let roofed = depth > CAVE_DEPTH - 1.0 && vw.raycast(p + up * 1.2, up, 30.0).is_some();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        StoryCtx { depth, roofed, kind: def.kind, day, relay, in_oxygen, now }
    }

    /// Standing on the surface at noon, breathing at the base.
    pub fn surface(kind: PlanetKind) -> StoryCtx {
        StoryCtx { depth: 0.0, roofed: false, kind, day: 1.0, relay: false, in_oxygen: true, now: 0 }
    }
}

/// Something `check` wants the player to notice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Note {
    Journal(u16),
    Say(L2),
}

impl Note {
    pub fn show(&self, ui: &mut crate::Ui, l: Lang) {
        match *self {
            Note::Journal(id) => ui.toast(format!("{}: {} (J)", tx(l, ("MİRA günlüğü", "MIRA's journal")), tx(l, JOURNAL[id as usize].title)), [1.0, 0.8, 0.5]),
            Note::Say(text) => ui.toast(format!("{}: {}", tx(l, ("MİRA", "MIRA")), tx(l, text)), [0.75, 0.9, 1.0]),
        }
    }
}

struct Out(Vec<Note>);

impl Out {
    fn entry(&mut self, game: &mut Game, id: u16) -> bool {
        let new = unlock(game, id);
        if new {
            self.0.push(Note::Journal(id));
        }
        new
    }
    fn once(&mut self, game: &mut Game, r: u8) {
        if !game.story.said.contains(&r) {
            game.story.said.push(r);
            self.0.push(Note::Say(REMARKS[r as usize]));
        }
    }
}

/// Resets the per-chapter memory when the player has reached a new star system.
fn sync_chapter(game: &mut Game) {
    if game.story.chapter == game.system_index {
        return;
    }
    let home = StarSystem::generate(game.seed, game.system_index).home;
    let s = &mut game.story;
    s.chapter = game.system_index;
    s.home = home;
    s.cave = false;
    s.relay = false;
    s.decoded = 0;
    s.nights = 0;
    s.left_home = false;
}

fn built_entry(k: SK) -> Option<u16> {
    Some(match k {
        SK::Smelter => J_SMELTER,
        SK::Research => J_RESEARCH,
        SK::ChemLab => J_CHEM,
        SK::Shuttle => J_SHUTTLE,
        SK::Shipyard => J_YARD,
        SK::Fabricator => J_FAB,
        SK::Solar | SK::BioGen => J_POWER,
        SK::Rtg => J_RTG,
        _ => return None,
    })
}

/// The story's per-frame trigger check. Unlocks journal entries and says remarks; returns what
/// the player should be told (show each with [`Note::show`]). Pure apart from `game`.
pub fn check(game: &mut Game, ctx: &StoryCtx) -> Vec<Note> {
    sync_chapter(game);
    let mut out = Out(Vec::new());
    let first = !game.story.awake;
    game.story.awake = true;
    let home = game.story.home;

    if game.journal.is_empty() {
        out.entry(game, J_AWAKE);
        out.entry(game, J_TOOL);
    }

    // back after a long time away (real time, across sessions)
    if first && game.story.wall > 0 && ctx.now > game.story.wall + ABSENCE_SECS {
        out.0.push(Note::Say(REMARKS[R_ABSENCE as usize]));
    }
    if ctx.now > 0 {
        game.story.wall = ctx.now;
    }

    // hooks fired by other systems
    for ev in std::mem::take(&mut game.story.pending) {
        match ev {
            StoryEvent::HazardHit => {
                game.story.hazard_hits += 1;
                out.entry(game, J_HURT);
            }
            StoryEvent::Blackout => {
                out.entry(game, J_BLACKOUT);
            }
            StoryEvent::WreckFound { id } => wreck(game, &mut out, id),
        }
    }
    // scrap only comes from wreckage: a fallback for sites nobody reported
    if game.collected_count(Item::Scrap) > 0 {
        wreck(game, &mut out, u32::MAX);
    }

    // underground
    if ctx.roofed && ctx.depth >= CAVE_DEPTH {
        game.story.cave = true;
        out.entry(game, J_CAVE);
    }
    if ctx.roofed && ctx.depth >= DEEP_DEPTH {
        out.entry(game, J_DEEP);
    }
    if ctx.relay {
        game.story.relay = true;
        out.entry(game, J_RELAY);
    }

    // hurt, blacked out, short of air
    if game.health < game.story.health - HIT && game.oxygen > 0.0 {
        out.entry(game, J_HURT);
    }
    game.story.health = game.health;
    if game.deaths >= 1 {
        out.entry(game, J_BLACKOUT);
    }
    if game.deaths >= 3 {
        out.once(game, R_CARRIED);
    }
    if !ctx.in_oxygen && game.oxygen > 0.0 && game.oxygen < 25.0 {
        out.once(game, R_LOW_O2);
    }

    // nights (with a little hysteresis so dusk does not flicker)
    let night = if game.story.night { ctx.day < 0.25 } else { ctx.day < 0.05 };
    if first {
        game.story.night = night;
    } else if night != game.story.night {
        game.story.night = night;
        if night {
            nightfall(game, &mut out);
        } else if game.story.nights > 0 {
            out.once(game, R_DAWN);
        }
    }
    if night && game.time > 20.0 {
        out.entry(game, J_NIGHT);
    }

    // where we are
    if let Some(i) = KIND_ORDER.iter().position(|k| *k == ctx.kind) {
        out.entry(game, J_KIND0 + i as u16);
    }
    if game.planet != home {
        game.story.left_home = true;
        if game.travels > 0 {
            out.entry(game, J_TRAVEL);
        }
        if game.planet.abs_diff(home) >= 2 && ctx.depth < 4.0 {
            out.once(game, R_FAR);
        }
    } else if game.story.left_home {
        game.story.left_home = false;
        out.entry(game, J_HOME);
    }
    match game.system_index {
        0 => {}
        1 => {
            out.entry(game, J_NEXT);
        }
        2 => {
            out.entry(game, J_CH3);
        }
        _ => {
            out.entry(game, J_OPEN);
        }
    }

    // what we built and made
    for k in game.built.clone() {
        if let Some(id) = built_entry(k) {
            out.entry(game, id);
        }
    }
    if game.discovered.contains(&Item::Heliosite) {
        out.entry(game, J_HELIO);
    }
    for (item, id) in [(Item::FusionCore, J_CORE), (Item::NavComputer, J_NAV), (Item::CryoPod, J_POD)] {
        if made(game, item) {
            out.entry(game, id);
        }
    }
    let yards = game.planets.iter().flat_map(|p| p.structs.iter()).filter(|s| s.kind == SK::Shipyard);
    let (started, complete) = yards.fold((false, false), |(a, b), s| (a || !s.delivered.is_empty(), b || s.ship_complete()));
    if started {
        out.once(game, R_FIRST_PART);
    }
    if complete {
        out.entry(game, J_SHIP);
    }
    out.0
}

/// A wreck site was found: the Pioneer's pieces tell their story in the first system, the
/// search fleet's lost probes in the later ones.
fn wreck(game: &mut Game, out: &mut Out, id: u32) {
    let key = (game.system_index as u64) << 40 | (game.planet as u64 & 0xff) << 32 | id as u64;
    if game.story.wrecks.contains(&key) {
        return;
    }
    game.story.wrecks.push(key);
    if game.system_index > 0 {
        out.entry(game, J_PROBE);
        return;
    }
    if game.planet != game.story.home && out.entry(game, J_ANTENNA) {
        return;
    }
    let order = [J_WRECK, J_CREW0, J_REACTOR, J_CREW0 + 1, J_CRYO, J_CREW0 + 2, J_CREW0 + 3];
    if let Some(&next) = order.iter().find(|id| !game.journal.contains(id)) {
        out.entry(game, next);
    }
}

/// Night fell. In the later chapters MIRA plays the messages she recorded for the other pods,
/// one per night, once the player knows why they exist.
fn nightfall(game: &mut Game, out: &mut Out) {
    game.story.nights += 1;
    if game.story.nights >= 5 {
        out.entry(game, J_NIGHTS);
    }
    if game.system_index >= 1 && game.journal.contains(&(J_FRAG0 + FRAG_COUNT as u16 - 1)) {
        if let Some(id) = (J_MSG0..J_MSG0 + MESSAGES).find(|id| !game.journal.contains(id)) {
            out.entry(game, id);
        }
    }
}

/// The order in which decoded fragments reveal logs in star system `system`: the black box
/// first (wherever its pieces turn up), then this chapter's logs, then anything left over.
pub fn log_order(system: u32) -> Vec<u16> {
    let chapters = [J_CREW0..J_CREW0 + CREW_LOGS, J_BEACON0..J_BEACON0 + CHAPTER_LOGS, J_NEAR0..J_NEAR0 + CHAPTER_LOGS];
    let cur = (system as usize).min(chapters.len() - 1);
    let mut v: Vec<u16> = (J_FRAG0..J_FRAG0 + FRAG_COUNT as u16).collect();
    v.extend(chapters[cur].clone());
    for c in chapters.iter().take(cur) {
        v.extend(c.clone());
    }
    v
}

/// A Research Station finished decoding a fragment. Returns the journal entry it revealed, or
/// None when every log reachable so far is known (the fragment still gave its Data).
pub fn decode_fragment(game: &mut Game) -> Option<u16> {
    sync_chapter(game);
    game.story.decoded += 1;
    let id = log_order(game.system_index).into_iter().find(|id| !game.journal.contains(id))?;
    if id < J_FRAG0 + FRAG_COUNT as u16 {
        game.fragments += 1;
    }
    unlock(game, id);
    Some(id)
}

/// The toast for a decoded log.
pub fn decode_toast(id: u16) -> L2 {
    if (J_FRAG0..J_FRAG0 + FRAG_COUNT as u16).contains(&id) {
        ("Kara kutu kaydı çözüldü! Günlüğe bak (J)", "Black box log decoded! Check the journal (J)")
    } else if (J_CREW0..J_CREW0 + CREW_LOGS).contains(&id) {
        ("Kişisel kayıt çözüldü! Günlüğe bak (J)", "Personal log decoded! Check the journal (J)")
    } else {
        ("İşaret kaydı çözüldü! Günlüğe bak (J)", "Beacon log decoded! Check the journal (J)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{Lang, SHIP_PARTS};
    use crate::state::Structure;

    fn collect(g: &mut Game, item: Item, n: u32) {
        match g.collected.iter_mut().find(|c| c.0 == item) {
            Some(c) => c.1 += n,
            None => g.collected.push((item, n)),
        }
        if !g.discovered.contains(&item) {
            g.discovered.push(item);
        }
    }

    fn build(g: &mut Game, kind: SK) -> u32 {
        let id = g.next_id;
        g.next_id += 1;
        g.structs_mut().push(Structure::new(id, kind, [0.0, 300.0, 0.0], [0.0, 0.0, 0.0, 1.0]));
        if !g.built.contains(&kind) {
            g.built.push(kind);
        }
        id
    }

    fn learn(g: &mut Game, t: Tech) {
        if !g.techs.contains(&t) {
            g.techs.push(t);
        }
    }

    fn ctx(g: &Game) -> StoryCtx {
        let sys = StarSystem::generate(g.seed, g.system_index);
        StoryCtx::surface(sys.planets[g.planet].kind)
    }

    /// One simulated frame: the story check, then the objective loop exactly as `simulate` runs it.
    fn frame(g: &mut Game, c: &StoryCtx) -> Vec<Note> {
        let notes = check(g, c);
        while g.quest < QUESTS.len() && (QUESTS[g.quest].done)(g) {
            g.quest += 1;
        }
        notes
    }

    fn fly(g: &mut Game, to: usize) {
        g.planet = to;
        g.travels += 1;
        g.planets[to].visited = true;
        let c = ctx(g);
        frame(g, &c);
    }

    /// Does what a player would do for objective `q`, using only what this star system offers.
    fn play(g: &mut Game, q: usize) {
        let sys = StarSystem::generate(g.seed, g.system_index);
        let has = |i: usize, item: Item| sys.planets[i].resources.iter().any(|r| r.0 == item);
        let mut c = ctx(g);
        match q {
            0 => collect(g, Item::Organic, 4),
            1 => {
                assert!(has(sys.home, Item::IronOre) && has(sys.home, Item::CopperOre));
                collect(g, Item::IronOre, 4);
                collect(g, Item::CopperOre, 2);
            }
            2 => {
                build(g, SK::Smelter);
            }
            3 => {
                collect(g, Item::Iron, 3);
                collect(g, Item::Copper, 1);
            }
            4 => {
                build(g, SK::Research);
            }
            5 => g.researched += 2,
            6 => {
                decode_fragment(g);
            }
            7 => {
                build(g, SK::Fabricator);
            }
            8 => {
                let b = build(g, SK::Beacon);
                g.find_struct(b).unwrap().links.push(1);
            }
            9 => c.relay = true,
            10 => {
                learn(g, Tech::Solar);
                build(g, SK::Solar);
            }
            11 => {
                c.depth = 12.0;
                c.roofed = true;
            }
            12 => {
                learn(g, Tech::ChemLab);
                build(g, SK::ChemLab);
            }
            13 => {
                learn(g, Tech::Steel);
                learn(g, Tech::Shuttle);
            }
            14 => {
                build(g, SK::Shuttle);
            }
            15 => {
                let to = (0..sys.planets.len()).find(|&i| i != sys.home && has(i, Item::TitaniumOre)).expect("titanium off home");
                fly(g, to);
                c = ctx(g);
            }
            16 => {
                let a = build(g, SK::Solar);
                let b = build(g, SK::Beacon);
                g.find_struct(b).unwrap().links.push(a);
            }
            17 => collect(g, Item::TitaniumOre, 2),
            18 => {
                learn(g, Tech::Titanium);
                learn(g, Tech::Shipyard);
                build(g, SK::Shipyard);
            }
            19 => {
                let to = (0..sys.planets.len()).find(|&i| has(i, Item::Heliosite)).expect("heliosite somewhere");
                fly(g, to);
                c = ctx(g);
                collect(g, Item::Heliosite, 7);
            }
            20 => {
                learn(g, Tech::ShipParts);
                collect(g, Item::FusionCore, 1);
            }
            21 => {
                let yard = g.planets.iter_mut().flat_map(|p| p.structs.iter_mut()).find(|s| s.kind == SK::Shipyard).unwrap();
                yard.delivered = SHIP_PARTS.to_vec();
            }
            22 => g.finished = true,
            _ => unreachable!(),
        }
        frame(g, &c);
    }

    #[test]
    fn quest_indices_match_their_objectives() {
        assert_eq!(QUESTS[Q_DELIVER].text.1, "Deliver every ship part and the fuel to the Shipyard");
        assert_eq!(Q_LAUNCH, QUESTS.len() - 1);
        assert_eq!(JOURNAL.len(), J_PROBE as usize + 1);
    }

    #[test]
    fn quests_complete_in_order_in_every_chapter() {
        for seed in [1u64, 7, 42, 1234, 99991] {
            let mut g = Game::new(seed, Lang::Tr);
            for system in 0..4u32 {
                assert_eq!(g.system_index, system);
                let c = ctx(&g);
                frame(&mut g, &c);
                assert_eq!(g.quest, 0, "seed {seed} system {system}: the first objective must not be done on arrival");
                for q in 0..QUESTS.len() {
                    if g.quest > q {
                        // already true from an earlier chapter (a researched tech); fine
                        continue;
                    }
                    assert_eq!(g.quest, q, "seed {seed} system {system}");
                    play(&mut g, q);
                    assert!(g.quest > q, "seed {seed} system {system}: objective {q} ({}) did not complete", QUESTS[q].text.1);
                }
                assert_eq!(g.quest, QUESTS.len());
                g.advance_system();
            }
        }
    }

    #[test]
    fn objectives_are_not_skipped_by_accident() {
        // a fresh game must walk the first chapter one objective at a time
        let mut g = Game::new(5, Lang::En);
        let c = ctx(&g);
        frame(&mut g, &c);
        for q in 0..QUESTS.len() {
            assert_eq!(g.quest, q);
            play(&mut g, q);
        }
    }

    /// Plays four chapters with every trigger the game has (objectives, decoding, hooks, caves,
    /// nights, planet kinds, coming home) and checks that every journal entry and remark turns up.
    #[test]
    fn every_journal_entry_and_remark_is_reachable() {
        let mut g = Game::new(42, Lang::Tr);
        let mut said: Vec<L2> = Vec::new();
        let run = |g: &mut Game, c: StoryCtx, said: &mut Vec<L2>| {
            for n in frame(g, &c) {
                if let Note::Say(s) = n {
                    said.push(s);
                }
            }
        };
        let mut c = ctx(&g);
        c.now = 1_000;
        run(&mut g, c, &mut said);
        for system in 0..4u32 {
            let sys = StarSystem::generate(g.seed, g.system_index);
            for q in 0..QUESTS.len() {
                if g.quest == q {
                    play(&mut g, q);
                }
                if q == 8 {
                    // decode fragments: black box, then this chapter's logs
                    for _ in 0..12 {
                        decode_fragment(&mut g);
                    }
                    // nights
                    for _ in 0..6 {
                        let mut c = ctx(&g);
                        c.day = 0.0;
                        g.time += 300.0;
                        run(&mut g, c, &mut said);
                        c.day = 1.0;
                        run(&mut g, c, &mut said);
                    }
                }
            }
            // wrecks: on the home planet and one elsewhere
            let far = (0..sys.planets.len()).max_by_key(|&i| i.abs_diff(sys.home)).unwrap();
            g.planet = sys.home;
            for id in 0..8 {
                fire(&mut g, StoryEvent::WreckFound { id });
            }
            let c = ctx(&g);
            run(&mut g, c, &mut said);
            g.planet = far;
            fire(&mut g, StoryEvent::WreckFound { id: 3 });
            fire(&mut g, StoryEvent::HazardHit);
            let mut c = ctx(&g);
            c.in_oxygen = false;
            g.oxygen = 20.0;
            run(&mut g, c, &mut said);
            g.oxygen = 100.0;
            // caves, deep
            let mut c = ctx(&g);
            c.depth = 60.0;
            c.roofed = true;
            run(&mut g, c, &mut said);
            // every kind of planet, then home again
            for kind in KIND_ORDER {
                run(&mut g, StoryCtx::surface(kind), &mut said);
            }
            g.planet = sys.home;
            let c = ctx(&g);
            run(&mut g, c, &mut said);
            build(&mut g, SK::Rtg);
            collect(&mut g, Item::NavComputer, 1);
            collect(&mut g, Item::CryoPod, 1);
            g.deaths += 2;
            let c = ctx(&g);
            run(&mut g, c, &mut said);
            if system < 3 {
                g.advance_system();
                let c = ctx(&g);
                run(&mut g, c, &mut said);
            }
        }
        // back after two days
        g.story.awake = false;
        let mut c = ctx(&g);
        c.now = g.story.wall + 2 * 86_400;
        run(&mut g, c, &mut said);
        for id in 0..JOURNAL.len() as u16 {
            assert!(g.journal.contains(&id), "journal entry {id} ({}) is never unlocked", JOURNAL[id as usize].title.1);
        }
        for (i, r) in REMARKS.iter().enumerate() {
            // some are said inside `play` (whose notes this test does not collect)
            assert!(said.contains(r) || g.story.said.contains(&(i as u8)), "remark never said: {}", r.1);
        }
        assert_eq!(g.fragments, FRAG_COUNT);
    }

    #[test]
    fn fragments_reveal_the_black_box_first_and_messages_wait_for_it() {
        // leaving the first system with only two black box logs: the third system still tells
        // the black box before its own logs, and the pod messages wait for the apology
        let mut g = Game::new(3, Lang::En);
        decode_fragment(&mut g);
        decode_fragment(&mut g);
        g.advance_system();
        g.advance_system();
        let c = ctx(&g);
        frame(&mut g, &c);
        let mut c = ctx(&g);
        c.day = 0.0;
        frame(&mut g, &c);
        assert!(!g.journal.contains(&J_MSG0), "messages need the apology first");
        let next: Vec<u16> = (0..7).filter_map(|_| decode_fragment(&mut g)).collect();
        assert_eq!(next[..6], [7, 8, 9, 10, 11, 12]);
        assert_eq!(next[6], J_NEAR0, "then this chapter's own logs");
        assert_eq!(g.fragments, FRAG_COUNT);
        c.day = 1.0;
        frame(&mut g, &c);
        c.day = 0.0;
        frame(&mut g, &c);
        assert!(g.journal.contains(&J_MSG0));
        // every log is in some chapter's order, once
        let mut all = log_order(3);
        let n = all.len();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), n);
        assert_eq!(n as u16, FRAG_COUNT as u16 + CREW_LOGS + 2 * CHAPTER_LOGS);
    }

    #[test]
    fn same_wreck_counts_once_and_later_systems_find_probes() {
        let mut g = Game::new(11, Lang::Tr);
        let c = ctx(&g);
        frame(&mut g, &c);
        for _ in 0..3 {
            fire(&mut g, StoryEvent::WreckFound { id: 77 });
        }
        frame(&mut g, &c);
        assert!(g.journal.contains(&J_WRECK));
        assert!(!g.journal.contains(&J_CREW0));
        g.advance_system();
        fire(&mut g, StoryEvent::WreckFound { id: 77 });
        let c = ctx(&g);
        frame(&mut g, &c);
        assert!(g.journal.contains(&J_PROBE));
    }

    #[test]
    fn a_pit_is_not_a_cave() {
        let mut g = Game::new(2, Lang::Tr);
        let mut c = ctx(&g);
        c.depth = 9.0;
        frame(&mut g, &c);
        assert!(!g.journal.contains(&J_CAVE));
        c.roofed = true;
        frame(&mut g, &c);
        assert!(g.journal.contains(&J_CAVE) && !g.journal.contains(&J_DEEP));
    }

    fn all_text() -> Vec<(String, &'static str)> {
        let mut v: Vec<(String, &'static str)> = Vec::new();
        let mut add = |what: String, s: L2| {
            v.push((what.clone(), s.0));
            v.push((what, s.1));
        };
        for (i, p) in INTRO.iter().chain(INTRO_2.iter()).chain(INTRO_3.iter()).chain(INTRO_OPEN.iter()).enumerate() {
            add(format!("intro page {i}"), *p);
        }
        for (i, e) in JOURNAL.iter().enumerate() {
            add(format!("journal {i} title"), e.title);
            add(format!("journal {i}"), e.body);
        }
        for s in 0..4 {
            add(format!("ending {s}"), ending(s));
        }
        for (i, q) in QUESTS.iter().enumerate() {
            add(format!("quest {i}"), q.text);
        }
        for (i, r) in REMARKS.iter().enumerate() {
            add(format!("remark {i}"), *r);
        }
        for id in [J_FRAG0, J_CREW0, J_BEACON0] {
            add(format!("toast {id}"), decode_toast(id));
        }
        v
    }

    /// Only MİRA / MIRA has a name. Every capitalised word that does not start a sentence must be
    /// a known game term, place word or role marker; anything else would be a name.
    #[test]
    fn no_names_but_mira() {
        const ALLOWED: &[&str] = &[
            "MİRA", "MIRA", "Öncü", "Pioneer", "II", "Dünya", "Earth", "Güneş", "Sun", "T",
            // game terms (items, structures, techs, UI)
            "Kazı", "Aleti", "Aletini", "Excavator", "Organik", "Organic", "Demir", "Bakır", "Cevheri", "Iron", "Copper", "Ore", "Eritme", "Ocağı", "Ocağında", "Smelter",
            "Araştırma", "İstasyonu", "İstasyonunu", "İstasyonundan", "İstasyonuna", "Kuvars", "Quartz", "Research", "Station", "Numuneler", "Specimens", "Üretim", "Tezgahı", "Tezgahında", "Assembler", "Oksijen", "Oxygen",
            "Halatı", "Halatlarını", "Relay", "Relays", "Güç", "Direği", "Power", "Pylon", "Güneş", "Enerjisini", "Paneli", "Panelleri", "Solar", "Panel", "Panels", "Biyo-Jeneratör", "Bio-Generator",
            "Kimya", "Laboratuvarı", "Chemistry", "Lab", "Mekik", "Mekiği", "Mekiğe", "Shuttle", "Yakıt", "Fuel", "Titanyum", "Titanium", "Sistem", "Haritasına", "Haritasından", "System", "Map",
            "Yıldız", "Tersanesini", "Tersanesi", "Star", "Shipyard", "Tersaneye", "Helyosit", "Heliosite", "Füzyon", "Çekirdeği", "Fusion", "Core", "Seyir", "Bilgisayarı", "Nav", "Computer",
            "Kriyo", "Kapsülü", "Kapsül", "Cryo", "Pod", "Gövde", "Bölümü", "Hull", "Sections", "Radyoizotop", "Jeneratör", "Radioisotope", "Generator", "Kara", "Kutu", "Parçası", "Black", "Box", "Fragment",
            "Veri", "Data", "Görev", "İzleme", "Rehberlik", "Asistanı", "Mission", "Monitoring", "Guidance", "Assistant", "Build", "Research", "Steel", "Çelik", "J",
            // pods and log markers
            "Bir", "İki", "Dört", "One", "Two", "Four", "İKİ'YE", "DÖRT'E", "BİR'E",
            "Komutan", "Commander", "Mühendis", "Engineer", "Pilot", "Doktor", "Doctor", "Doc", "Kişisel", "Kayıt", "Personal", "Log", "Message", "Mesaj",
            "İşaret", "Kaydı", "Beacon", "Yakın", "Near", "Kara", "Kutu", "X-rays",
        ];
        const DENY: &[&str] = &["Ahmet", "Mehmet", "Ayşe", "Fatma", "Ali", "Elif", "John", "Sarah", "Anna", "Alex", "Captain", "Kaptan"];
        let mut bad = Vec::new();
        for (what, text) in all_text() {
            let chars: Vec<char> = text.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                if chars[i].is_alphabetic() && (i == 0 || !chars[i - 1].is_alphanumeric() && chars[i - 1] != '\'') {
                    let start = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '\'') {
                        i += 1;
                    }
                    let word: String = chars[start..i].iter().collect();
                    let stem = word.split('\'').next().unwrap().trim_end_matches('-').to_string();
                    if DENY.contains(&stem.as_str()) {
                        bad.push(format!("{what}: {word}"));
                    }
                    // sentence start: only spaces / quotes / brackets back to . ! ? : or a line break
                    let mut j = start;
                    while j > 0 && matches!(chars[j - 1], ' ' | '"' | '\'' | '[' | '(') {
                        j -= 1;
                    }
                    let sentence_start = j == 0 || matches!(chars[j - 1], '.' | '!' | '?' | ':' | '\n' | ';');
                    let upper = stem.chars().next().is_some_and(|c| c.is_uppercase());
                    let all_caps = stem.chars().all(|c| !c.is_lowercase());
                    // titles are in title case: only the deny list applies to them
                    let title = what.ends_with("title");
                    if upper && !title && !sentence_start && !all_caps && !ALLOWED.contains(&stem.as_str()) && !ALLOWED.contains(&word.as_str()) {
                        bad.push(format!("{what}: {word}"));
                    }
                } else {
                    i += 1;
                }
            }
        }
        assert!(bad.is_empty(), "capitalised words that may be names:\n{}", bad.join("\n"));
    }

    #[test]
    fn every_text_has_both_languages_and_placeholders_match() {
        for (what, s) in all_text() {
            assert!(!s.trim().is_empty(), "{what} is empty");
        }
        let both = |a: &str, b: &str, what: &str| {
            for ph in ["{star}", "{planet}"] {
                assert_eq!(a.contains(ph), b.contains(ph), "{what}: {ph} in one language only");
            }
        };
        for (i, e) in JOURNAL.iter().enumerate() {
            both(e.body.0, e.body.1, &format!("journal {i}"));
        }
        for p in INTRO.iter().chain(INTRO_2.iter()).chain(INTRO_3.iter()).chain(INTRO_OPEN.iter()) {
            both(p.0, p.1, "intro");
        }
    }
}
