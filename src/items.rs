//! Static game data: items, recipes, structures, technologies + the inventory container.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Lang {
    Tr,
    En,
}

/// Bilingual text: (Turkish, English)
pub type L2 = (&'static str, &'static str);

pub fn tx(l: Lang, s: L2) -> &'static str {
    match l {
        Lang::Tr => s.0,
        Lang::En => s.1,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Item {
    Organic,
    IronOre,
    CopperOre,
    Quartz,
    Carbon,
    Ice,
    TitaniumOre,
    Lithium,
    Uranite,
    Heliosite,
    Iron,
    Copper,
    Glass,
    Silicon,
    Steel,
    Plastic,
    Titanium,
    Fuel,
    Cell,
    FuelRod,
    Circuit,
    Motor,
    Plating,
    HullSection,
    NavComputer,
    CryoPod,
    FusionCore,
    Sample,
    DataFragment,
    Scrap,
}

pub struct ItemDef {
    pub name: L2,
    pub color: [f32; 3],
    pub desc: L2,
}

impl Item {
    pub const ALL: [Item; 30] = [
        Item::Organic,
        Item::IronOre,
        Item::CopperOre,
        Item::Quartz,
        Item::Carbon,
        Item::Ice,
        Item::TitaniumOre,
        Item::Lithium,
        Item::Uranite,
        Item::Heliosite,
        Item::Iron,
        Item::Copper,
        Item::Glass,
        Item::Silicon,
        Item::Steel,
        Item::Plastic,
        Item::Titanium,
        Item::Fuel,
        Item::Cell,
        Item::FuelRod,
        Item::Circuit,
        Item::Motor,
        Item::Plating,
        Item::HullSection,
        Item::NavComputer,
        Item::CryoPod,
        Item::FusionCore,
        Item::Sample,
        Item::DataFragment,
        Item::Scrap,
    ];

    pub fn def(self) -> ItemDef {
        use Item::*;
        let (name, color, desc): (L2, [f32; 3], L2) = match self {
            Organic => (("Organik", "Organic"), [0.35, 0.7, 0.25], ("Bitkilerden toplanır. Yakıt, plastik ve kömür için.", "Harvested from plants. Used for fuel, plastic and charcoal.")),
            IronOre => (("Demir Cevheri", "Iron Ore"), [0.62, 0.33, 0.25], ("Eritilerek demir elde edilir.", "Smelts into iron.")),
            CopperOre => (("Bakır Cevheri", "Copper Ore"), [0.2, 0.62, 0.5], ("Eritilerek bakır elde edilir.", "Smelts into copper.")),
            Quartz => (("Kuvars", "Quartz"), [0.92, 0.9, 0.95], ("Cam ve silisyumun ham maddesi.", "Raw material for glass and silicon.")),
            Carbon => (("Karbon", "Carbon"), [0.16, 0.16, 0.18], ("Çelik, silisyum ve yakıt üretiminde kullanılır.", "Used for steel, silicon and fuel.")),
            Ice => (("Buz", "Ice"), [0.88, 0.97, 0.98], ("Su buzu. Roket yakıtının temel bileşeni.", "Water ice. The base of rocket fuel.")),
            TitaniumOre => (("Titanyum Cevheri", "Titanium Ore"), [0.55, 0.6, 0.75], ("Hafif ve dayanıklı titanyumun cevheri.", "Ore of light, strong titanium.")),
            Lithium => (("Lityum", "Lithium"), [0.95, 0.6, 0.75], ("Enerji hücrelerinin ana maddesi.", "Main ingredient of power cells.")),
            Uranite => (("Uranit", "Uranite"), [0.45, 0.95, 0.3], ("Radyoaktif mineral. Yakıt çubuğuna işlenir.", "Radioactive mineral. Processed into fuel rods.")),
            Heliosite => (("Helyosit", "Heliosite"), [1.0, 0.8, 0.25], ("Yıldız ışığını hapseden egzotik kristal. Füzyon çekirdeği için şart.", "Exotic crystal that traps starlight. Required for the fusion core.")),
            Iron => (("Demir", "Iron"), [0.7, 0.7, 0.72], ("Temel yapı malzemesi.", "Basic construction material.")),
            Copper => (("Bakır", "Copper"), [0.85, 0.5, 0.3], ("İletken metal.", "Conductive metal.")),
            Glass => (("Cam", "Glass"), [0.7, 0.9, 0.95], ("Paneller ve kubbeler için.", "For panels and domes.")),
            Silicon => (("Silisyum", "Silicon"), [0.4, 0.45, 0.6], ("Elektronik için saf silisyum.", "Pure silicon for electronics.")),
            Steel => (("Çelik", "Steel"), [0.5, 0.55, 0.62], ("Demir ve karbon alaşımı.", "Alloy of iron and carbon.")),
            Plastic => (("Plastik", "Plastic"), [0.95, 0.95, 0.85], ("Organik polimer.", "Organic polymer.")),
            Titanium => (("Titanyum", "Titanium"), [0.75, 0.8, 0.9], ("Uzay sınıfı metal.", "Space-grade metal.")),
            Fuel => (("Yakıt", "Fuel"), [0.95, 0.45, 0.15], ("Roket yakıtı.", "Rocket propellant.")),
            Cell => (("Enerji Hücresi", "Power Cell"), [0.3, 0.9, 0.9], ("Lityum tabanlı enerji deposu.", "Lithium based energy storage.")),
            FuelRod => (("Yakıt Çubuğu", "Fuel Rod"), [0.5, 1.0, 0.4], ("Zenginleştirilmiş uranyum çubuğu.", "Enriched uranium rod.")),
            Circuit => (("Devre Kartı", "Circuit Board"), [0.15, 0.6, 0.3], ("Her makinenin beyni.", "The brain of every machine.")),
            Motor => (("Motor", "Motor"), [0.55, 0.5, 0.45], ("Elektrik motoru.", "Electric motor.")),
            Plating => (("Gövde Plakası", "Hull Plating"), [0.8, 0.82, 0.86], ("Çelik-titanyum kompozit plaka.", "Steel-titanium composite plate.")),
            HullSection => (("Gövde Bölümü", "Hull Section"), [0.9, 0.9, 0.95], ("Yıldız gemisinin gövde parçası.", "A section of the starship hull.")),
            NavComputer => (("Seyir Bilgisayarı", "Nav Computer"), [0.3, 0.5, 1.0], ("Yıldızlararası rota hesaplar.", "Computes interstellar courses.")),
            CryoPod => (("Kriyo Kapsül", "Cryo Pod"), [0.6, 0.95, 1.0], ("Uzun yolculuk için uyku kapsülü.", "Sleep pod for the long journey.")),
            FusionCore => (("Füzyon Çekirdeği", "Fusion Core"), [1.0, 0.55, 0.9], ("Helyosit beslemeli füzyon motorunun kalbi.", "Heart of the heliosite-fed fusion drive.")),
            Sample => (("Numune", "Specimen"), [0.4, 0.7, 1.0], ("Araştırma İstasyonunda incelenir.", "Analyse it at a Research Station.")),
            DataFragment => (("Kara Kutu Parçası", "Black Box Fragment"), [1.0, 0.35, 0.3], ("Geminin kayıtlarından bir parça. Araştırma İstasyonunda çözülür.", "A piece of the ship's logs. Decode it at a Research Station.")),
            Scrap => (("Hurda", "Scrap"), [0.52, 0.44, 0.4], ("Enkaz parçası. Eritilerek demire dönüşür.", "Wreckage. Smelts down into iron.")),
        };
        ItemDef { name, color, desc }
    }

    pub fn name(self, l: Lang) -> &'static str {
        tx(l, self.def().name)
    }
}

/// Raw resources that occur in planet crusts: (item, min depth, noise frequency)
pub const ORES: [(Item, f32, f32); 9] = [
    (Item::IronOre, 1.5, 0.085),
    (Item::CopperOre, 2.5, 0.09),
    (Item::Quartz, 0.5, 0.1),
    (Item::Carbon, 3.0, 0.08),
    (Item::Ice, 0.5, 0.07),
    (Item::TitaniumOre, 3.0, 0.085),
    (Item::Lithium, 2.0, 0.09),
    (Item::Uranite, 6.0, 0.095),
    (Item::Heliosite, 7.0, 0.1),
];

pub const STACK: u32 = 40;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub slots: Vec<Option<(Item, u32)>>,
}

impl Inventory {
    pub fn new(n: usize) -> Self {
        Inventory { slots: vec![None; n] }
    }
    pub fn count(&self, item: Item) -> u32 {
        self.slots.iter().flatten().filter(|s| s.0 == item).map(|s| s.1).sum()
    }
    /// Returns the amount that did not fit.
    pub fn add(&mut self, item: Item, mut n: u32) -> u32 {
        for s in self.slots.iter_mut().flatten() {
            if s.0 == item && s.1 < STACK {
                let k = n.min(STACK - s.1);
                s.1 += k;
                n -= k;
                if n == 0 {
                    return 0;
                }
            }
        }
        for s in self.slots.iter_mut() {
            if s.is_none() {
                let k = n.min(STACK);
                *s = Some((item, k));
                n -= k;
                if n == 0 {
                    return 0;
                }
            }
        }
        n
    }
    pub fn remove(&mut self, item: Item, mut n: u32) -> bool {
        if self.count(item) < n {
            return false;
        }
        for s in self.slots.iter_mut().rev() {
            if let Some((it, c)) = s {
                if *it == item {
                    let k = n.min(*c);
                    *c -= k;
                    n -= k;
                    if *c == 0 {
                        *s = None;
                    }
                    if n == 0 {
                        break;
                    }
                }
            }
        }
        true
    }
    pub fn has_all(&self, req: &[(Item, u32)]) -> bool {
        req.iter().all(|&(i, n)| self.count(i) >= n)
    }
    pub fn remove_all(&mut self, req: &[(Item, u32)]) -> bool {
        if !self.has_all(req) {
            return false;
        }
        for &(i, n) in req {
            self.remove(i, n);
        }
        true
    }
    pub fn free_slots(&self) -> usize {
        self.slots.iter().filter(|s| s.is_none()).count()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Tech {
    Storage,
    Solar,
    Steel,
    WideTool,
    Battery,
    ChemLab,
    Scanner,
    Backpack,
    Jetpack,
    Shuttle,
    Titanium,
    Rtg,
    Shipyard,
    ShipParts,
}

pub struct TechDef {
    pub name: L2,
    pub desc: L2,
    pub cost: u32,
    pub prereq: Option<Tech>,
}

impl Tech {
    pub const ALL: [Tech; 14] = [
        Tech::Storage,
        Tech::Solar,
        Tech::Steel,
        Tech::WideTool,
        Tech::Battery,
        Tech::ChemLab,
        Tech::Scanner,
        Tech::Backpack,
        Tech::Jetpack,
        Tech::Shuttle,
        Tech::Titanium,
        Tech::Rtg,
        Tech::Shipyard,
        Tech::ShipParts,
    ];
    pub fn def(self) -> TechDef {
        use Tech::*;
        let (name, desc, cost, prereq): (L2, L2, u32, Option<Tech>) = match self {
            Storage => (("Depolama", "Storage"), ("Depo Sandığı inşa etmeyi açar.", "Unlocks the Storage Crate."), 60, None),
            Solar => (("Güneş Enerjisi", "Solar Power"), ("Güneş Paneli inşa etmeyi açar.", "Unlocks the Solar Panel."), 120, None),
            Steel => (("Çelik Dökümü", "Steelmaking"), ("Eritme Ocağında çelik üretimi.", "Steel recipe in the Smelter."), 150, None),
            WideTool => (("Güçlendirilmiş Kazı Aleti", "Boosted Excavator"), ("Daha geniş ve hızlı kazı.", "Wider and faster digging."), 150, None),
            Battery => (("Batarya", "Battery"), ("Fazla enerjiyi depolayan batarya.", "A battery that stores surplus power."), 180, Some(Solar)),
            ChemLab => (("Kimya", "Chemistry"), ("Kimya Laboratuvarı: plastik, yakıt, hücre.", "Chemistry Lab: plastic, fuel, cells."), 200, None),
            Scanner => (("Maden Tarayıcı", "Ore Scanner"), ("HUD'da en yakın maden yataklarını gösterir.", "Shows the nearest deposits on the HUD."), 200, None),
            Backpack => (("Geniş Sırt Çantası", "Bigger Backpack"), ("+8 envanter yuvası.", "+8 inventory slots."), 250, Some(Storage)),
            Jetpack => (("Jetpack", "Jetpack"), ("Havada Zıpla tuşunu basılı tutarak uç.", "Hold Jump in mid-air to fly."), 300, Some(ChemLab)),
            Shuttle => (("Mekik", "Shuttle"), ("Sistemdeki diğer gezegenlere uçan mekik.", "A shuttle to reach other planets."), 500, Some(ChemLab)),
            Titanium => (("Titanyum İşleme", "Titanium Processing"), ("Titanyum eritme ve gövde plakası.", "Titanium smelting and hull plating."), 400, Some(Shuttle)),
            Rtg => (("Radyoizotop Jeneratör", "Radioisotope Generator"), ("Yakıt çubuğu ve kesintisiz güç.", "Fuel rods and uninterrupted power."), 600, Some(Titanium)),
            Shipyard => (("Yıldız Tersanesi", "Star Shipyard"), ("Yıldızlararası geminin iskelesi.", "The gantry of the interstellar ship."), 900, Some(Titanium)),
            ShipParts => (("Yıldız Gemisi Sistemleri", "Starship Systems"), ("Füzyon çekirdeği, seyir bilgisayarı, kriyo kapsül, gövde.", "Fusion core, nav computer, cryo pod, hull."), 1200, Some(Shipyard)),
        };
        TechDef { name, desc, cost, prereq }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum SK {
    Pod,
    Smelter,
    Fabricator,
    ChemLab,
    Research,
    Solar,
    BioGen,
    Battery,
    Storage,
    Beacon,
    Shuttle,
    Shipyard,
    Rtg,
    Tether,
    Pylon,
}

pub struct StructDef {
    pub name: L2,
    pub desc: L2,
    pub cost: &'static [(Item, u32)],
    pub tech: Option<Tech>,
    /// positive = generates, negative = consumes while working
    pub power: f32,
    pub radius: f32,
    pub oxygen: f32,
}

impl SK {
    pub const BUILDABLE: [SK; 14] = [
        SK::Tether,
        SK::Pylon,
        SK::Smelter,
        SK::Research,
        SK::Fabricator,
        SK::Beacon,
        SK::BioGen,
        SK::Storage,
        SK::Solar,
        SK::Battery,
        SK::ChemLab,
        SK::Shuttle,
        SK::Rtg,
        SK::Shipyard,
    ];
    pub fn def(self) -> StructDef {
        use Item::*;
        let d = |name, desc, cost, tech, power, radius, oxygen| StructDef { name, desc, cost, tech, power, radius, oxygen };
        match self {
            SK::Pod => d(("Kurtarma Kapsülü", "Escape Pod"), ("Enkazdan sağlam kalan tek modül. Oksijen ve az miktarda güç sağlar.", "The only module that survived. Provides oxygen and a little power."), &[], None, 4.0, 2.2, 10.0),
            SK::Smelter => d(("Eritme Ocağı", "Smelter"), ("Cevherleri metale ve cama dönüştürür.", "Turns ores into metal and glass."), &[(IronOre, 4), (CopperOre, 2)], None, -4.0, 1.4, 0.0),
            SK::Research => d(("Araştırma İstasyonu", "Research Station"), ("Numuneleri ve kara kutu parçalarını Veri'ye çevirir.", "Turns specimens and black box fragments into Data."), &[(Iron, 2), (Copper, 1), (Quartz, 2)], None, -3.0, 1.4, 0.0),
            SK::Fabricator => d(("Üretim Tezgahı", "Assembler"), ("Devre, motor ve gemi parçaları üretir.", "Builds circuits, motors and ship parts."), &[(Iron, 3), (Copper, 2)], None, -4.0, 1.5, 0.0),
            SK::Beacon => d(("Oksijen İstasyonu", "Oxygen Station"), ("Güç kablosuna bağlıyken çevresinde nefes alınabilir bir alan oluşturur ve halatları besler.", "While cabled to power, creates a breathable zone and feeds relays."), &[(Iron, 1), (Glass, 1)], None, -1.0, 0.5, 14.0),
            SK::BioGen => d(("Biyo-Jeneratör", "Bio-Generator"), ("Organik yakarak güç üretir.", "Burns organic matter for power."), &[(Iron, 2), (Copper, 1)], None, 8.0, 1.0, 0.0),
            SK::Storage => d(("Depo Sandığı", "Storage Crate"), ("24 yuvalı depo.", "A 24-slot container."), &[(Iron, 3)], Some(Tech::Storage), 0.0, 1.0, 0.0),
            SK::Solar => d(("Güneş Paneli", "Solar Panel"), ("Gündüzleri güç üretir.", "Generates power in daylight."), &[(Glass, 2), (Copper, 1), (Silicon, 1)], Some(Tech::Solar), 6.0, 1.2, 0.0),
            SK::Battery => d(("Batarya", "Battery"), ("Fazla gücü depolar, gece geri verir.", "Stores surplus power for the night."), &[(Copper, 2), (Carbon, 2), (Iron, 1)], Some(Tech::Battery), 0.0, 0.8, 0.0),
            SK::ChemLab => d(("Kimya Laboratuvarı", "Chemistry Lab"), ("Plastik, yakıt ve enerji hücresi üretir.", "Produces plastic, fuel and power cells."), &[(Iron, 3), (Glass, 3), (Copper, 2)], Some(Tech::ChemLab), -5.0, 1.5, 0.0),
            SK::Shuttle => d(("Mekik", "Shuttle"), ("Yakıt doldur, bin ve başka bir gezegene uç.", "Fuel it, board it and fly to another planet."), &[(Steel, 6), (Motor, 2), (Circuit, 2), (Glass, 2), (Plastic, 2)], Some(Tech::Shuttle), 0.0, 1.8, 6.0),
            SK::Rtg => d(("Radyoizotop Jeneratör", "Radioisotope Generator"), ("Gece gündüz kesintisiz güç.", "Constant power, day and night."), &[(FuelRod, 2), (Steel, 3), (Circuit, 1)], Some(Tech::Rtg), 20.0, 1.0, 0.0),
            SK::Shipyard => d(("Yıldız Tersanesi", "Star Shipyard"), ("Yıldızlararası gemi burada parça parça inşa edilir.", "The interstellar ship is assembled here, piece by piece."), &[(Steel, 8), (Titanium, 4), (Motor, 3), (Circuit, 3)], Some(Tech::Shipyard), 0.0, 4.0, 8.0),
            SK::Tether => d(("Oksijen Halatı", "Oxygen Relay"), ("Bir oksijen kaynağına zincirlenen ucuz direk. Zincir canlıyken yakınında nefes alırsın. Kısayol: T / HALAT.", "A cheap post that chains back to an oxygen source; breathe near it while the chain is live. Shortcut: T / RELAY."), &[(Organic, 1)], None, 0.0, 0.25, 0.0),
            SK::Pylon => d(("Güç Direği", "Power Pylon"), ("Güç kablolarını uzağa taşır (26 m'ye kadar).", "Carries power cables over long distances (up to 26 m)."), &[(Iron, 1)], None, 0.0, 0.3, 0.0),
        }
    }
    pub fn name(self, l: Lang) -> &'static str {
        tx(l, self.def().name)
    }
    pub fn station(self) -> Option<Station> {
        match self {
            SK::Smelter => Some(Station::Smelter),
            SK::Fabricator => Some(Station::Fabricator),
            SK::ChemLab => Some(Station::ChemLab),
            SK::Research => Some(Station::Research),
            _ => None,
        }
    }
}

/// What the finished starship needs.
pub const SHIP_PARTS: [(Item, u32); 5] = [
    (Item::HullSection, 4),
    (Item::FusionCore, 1),
    (Item::NavComputer, 1),
    (Item::CryoPod, 1),
    (Item::Fuel, 20),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Station {
    Smelter,
    Fabricator,
    ChemLab,
    Research,
}

pub struct Recipe {
    pub station: Station,
    pub inputs: &'static [(Item, u32)],
    pub output: (Item, u32),
    pub time: f32,
    pub tech: Option<Tech>,
    /// research recipes give Data instead of an item
    pub data: u32,
}

const fn r(station: Station, inputs: &'static [(Item, u32)], output: (Item, u32), time: f32, tech: Option<Tech>) -> Recipe {
    Recipe { station, inputs, output, time, tech, data: 0 }
}

use Item::*;
pub static RECIPES: [Recipe; 23] = [
    r(Station::Smelter, &[(IronOre, 2)], (Iron, 1), 4.0, None),
    r(Station::Smelter, &[(CopperOre, 2)], (Copper, 1), 4.0, None),
    r(Station::Smelter, &[(Quartz, 2)], (Glass, 1), 4.0, None),
    r(Station::Smelter, &[(Organic, 3)], (Carbon, 1), 5.0, None),
    r(Station::Smelter, &[(Quartz, 2), (Carbon, 1)], (Silicon, 1), 6.0, None),
    r(Station::Smelter, &[(Iron, 2), (Carbon, 1)], (Steel, 1), 7.0, Some(Tech::Steel)),
    r(Station::Smelter, &[(TitaniumOre, 2)], (Titanium, 1), 8.0, Some(Tech::Titanium)),
    r(Station::Fabricator, &[(Copper, 1), (Silicon, 1)], (Circuit, 1), 6.0, None),
    r(Station::Fabricator, &[(Iron, 2), (Copper, 1)], (Motor, 1), 6.0, None),
    r(Station::Fabricator, &[(Steel, 1), (Titanium, 1)], (Plating, 1), 8.0, Some(Tech::Titanium)),
    r(Station::Fabricator, &[(Plating, 2), (Steel, 2)], (HullSection, 1), 15.0, Some(Tech::ShipParts)),
    r(Station::Fabricator, &[(Circuit, 6), (Glass, 2), (Cell, 2)], (NavComputer, 1), 20.0, Some(Tech::ShipParts)),
    r(Station::Fabricator, &[(Plastic, 4), (Glass, 3), (Circuit, 2), (Cell, 1)], (CryoPod, 1), 20.0, Some(Tech::ShipParts)),
    r(Station::Fabricator, &[(Heliosite, 6), (Titanium, 4), (Circuit, 4), (FuelRod, 2)], (FusionCore, 1), 30.0, Some(Tech::ShipParts)),
    r(Station::ChemLab, &[(Organic, 2), (Carbon, 1)], (Plastic, 1), 5.0, None),
    r(Station::ChemLab, &[(Ice, 2), (Carbon, 1)], (Fuel, 1), 6.0, None),
    r(Station::ChemLab, &[(Organic, 5)], (Fuel, 1), 9.0, None),
    r(Station::ChemLab, &[(Lithium, 2), (Copper, 1), (Plastic, 1)], (Cell, 1), 8.0, None),
    r(Station::ChemLab, &[(Uranite, 3), (Steel, 1)], (FuelRod, 1), 12.0, None),
    Recipe { station: Station::Research, inputs: &[(Sample, 1)], output: (Sample, 0), time: 8.0, tech: None, data: 50 },
    Recipe { station: Station::Research, inputs: &[(DataFragment, 1)], output: (DataFragment, 0), time: 12.0, tech: None, data: 120 },
    Recipe { station: Station::Research, inputs: &[(Heliosite, 1)], output: (Heliosite, 0), time: 15.0, tech: None, data: 200 },
    r(Station::Smelter, &[(Scrap, 2)], (Iron, 1), 5.0, None),
];
