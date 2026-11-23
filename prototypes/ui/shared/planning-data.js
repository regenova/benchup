// BenchUp UI prototypes: workshop inventory and build plan for the sample hall bench.
// The workshop (tools you own, how you like to work) is a user preference shared by
// every project; the plan state (chosen methods, finished steps) belongs to the project.
(function () {
  "use strict";

  // ---------- Tool catalogue ----------
  const CATS = [
    { id: "mark", name: "Measuring & marking" },
    { id: "hand", name: "Hand tools" },
    { id: "power", name: "Power tools" },
    { id: "machine", name: "Stationary machines" },
    { id: "jig", name: "Bits, blades & jigs" },
    { id: "clamp", name: "Clamping & assembly" },
    { id: "finish", name: "Finishing" },
  ];

  const T = (id, cat, name, price, extra = {}) => Object.assign({ id, cat, name, price }, extra);
  const TOOLS = [
    T("tape", "mark", "Tape measure", 12),
    T("square", "mark", "Combination square", 35),
    T("mgauge", "mark", "Marking gauge", 25),
    T("mortgauge", "mark", "Mortise gauge", 45, { detail: "Two pins, set to the chisel" }),
    T("knife", "mark", "Marking knife", 18),
    T("labels", "mark", "Painter's tape or label sheets", 6),

    T("crosscut", "hand", "Crosscut handsaw", 35, { detail: "Ryoba or panel saw" }),
    T("tenonsaw", "hand", "Tenon saw", 60),
    T("chisels", "hand", "Bench chisels", 70, { detail: "6 to 25 mm" }),
    T("mortchisel", "hand", "Mortise chisel", 55, { detail: "Matching the 6 mm tenon" }),
    T("mallet", "hand", "Mallet", 20),
    T("blockplane", "hand", "Block plane", 60),
    T("jackplane", "hand", "Jack plane", 110, { detail: "No. 5" }),
    T("jointerplane", "hand", "Jointer plane", 190, { detail: "No. 7, for long edges" }),
    T("shoulderplane", "hand", "Shoulder plane", 120),
    T("routerplane", "hand", "Router plane", 130),
    T("ploughplane", "hand", "Plough plane", 220),
    T("scraper", "hand", "Card scraper", 12),
    T("rasp", "hand", "Rasp", 30),

    T("drill", "power", "Drill / driver", 110),
    T("router", "power", "Plunge router", 220, { detail: "8 or 12 mm collet" }),
    T("ros", "power", "Random orbital sander", 120),
    T("circsaw", "power", "Circular or track saw", 260),
    T("jigsaw", "power", "Jigsaw", 90),
    T("domino", "power", "Domino joiner", 1100, { detail: "Loose tenon joiner" }),

    T("tablesaw", "machine", "Table saw", 900),
    T("mitersaw", "machine", "Mitre saw", 280),
    T("jointer", "machine", "Jointer", 700, { detail: "At least 150 mm wide" }),
    T("planer", "machine", "Thickness planer", 550),
    T("bandsaw", "machine", "Bandsaw", 650),
    T("drillpress", "machine", "Drill press", 320),
    T("mortiser", "machine", "Hollow chisel mortiser", 450),
    T("routertable", "machine", "Router table", 250),

    T("forstner", "jig", "Forstner bits", 35),
    T("straightbit", "jig", "Straight router bits", 30),
    T("roundover", "jig", "Round-over bit, R6", 25),
    T("chamferbit", "jig", "Chamfer bit, 45°", 22),
    T("edgeguide", "jig", "Router edge guide", 40),
    T("tenonjig", "jig", "Tenoning jig", 110),
    T("dado", "jig", "Dado blade set", 140),
    T("shootboard", "jig", "Shooting board", 0, { detail: "Shop-made" }),

    T("barclamps", "clamp", "Bar clamps", 22, { qty: true, detail: "900 mm or longer" }),
    T("fclamps", "clamp", "F-clamps", 12, { qty: true }),
    T("strap", "clamp", "Ratchet straps", 9, { qty: true }),
    T("glue", "clamp", "Wood glue", 9),

    T("rags", "finish", "Lint-free rags and gloves", 8),
    T("brushes", "finish", "Finishing brushes", 15),
  ];
  const TOOL = Object.fromEntries(TOOLS.map((t) => [t.id, t]));

  const list = (ids) => Object.fromEntries(ids.map((x) => (Array.isArray(x) ? x : [x, 1])));
  const PRESETS = [
    {
      id: "starter",
      name: "Just starting",
      note: "A drill, a saw and a few clamps",
      owned: list(["tape", "square", "knife", "labels", "crosscut", "chisels", "mallet", "drill", "forstner", "jigsaw", "ros", ["fclamps", 2], "glue", "rags", "brushes"]),
    },
    {
      id: "hand",
      name: "Hand tools only",
      note: "Saws, chisels and planes, plus a drill",
      owned: list(["tape", "square", "mgauge", "mortgauge", "knife", "labels", "crosscut", "tenonsaw", "chisels", "mortchisel", "mallet", "blockplane", "jackplane", "jointerplane", "shoulderplane", "routerplane", "ploughplane", "scraper", "rasp", "shootboard", "drill", "forstner", ["barclamps", 4], ["fclamps", 4], "glue", "rags", "brushes"]),
    },
    {
      id: "small",
      name: "Small shop",
      note: "Portable power tools and a mitre saw",
      owned: list(["tape", "square", "mgauge", "knife", "labels", "crosscut", "tenonsaw", "chisels", "mallet", "blockplane", "jackplane", "rasp", "drill", "router", "ros", "circsaw", "jigsaw", "mitersaw", "forstner", "straightbit", "edgeguide", "roundover", ["barclamps", 2], ["fclamps", 4], "glue", "rags", "brushes"]),
    },
    {
      id: "full",
      name: "Full shop",
      note: "Milling machines, table saw and mortiser",
      owned: Object.fromEntries(TOOLS.map((t) => [t.id, t.id === "barclamps" ? 8 : t.id === "fclamps" ? 8 : t.id === "strap" ? 2 : 1])),
    },
  ];

  const STYLES = [
    { id: "hand", name: "Hand tools" },
    { id: "balanced", name: "Balanced" },
    { id: "machine", name: "Machines" },
  ];
  const LEVELS = [
    { id: "beginner", name: "Beginner", factor: 1.5 },
    { id: "intermediate", name: "Intermediate", factor: 1 },
    { id: "experienced", name: "Experienced", factor: 0.8 },
  ];

  // ---------- Storage ----------
  const SHOP_KEY = "benchup-proto-workshop";
  const PLAN_KEY = "benchup-proto-plan";
  const read = (key) => {
    try { return JSON.parse(localStorage.getItem(key) || "null"); } catch (_) { return null; }
  };
  const write = (key, value) => {
    try { localStorage.setItem(key, JSON.stringify(value)); } catch (_) {}
  };
  function loadShop() {
    const saved = read(SHOP_KEY);
    const small = PRESETS.find((p) => p.id === "small");
    const base = { preset: "small", owned: Object.assign({}, small.owned), style: "balanced", level: "intermediate" };
    return saved ? Object.assign(base, saved) : base;
  }
  const saveShop = (shop) => write(SHOP_KEY, shop);
  function loadPlan() {
    const base = { mode: "mine", view: "steps", methods: {}, done: {}, sel: "s-mortise" };
    const saved = read(PLAN_KEY);
    return saved ? Object.assign(base, saved) : base;
  }
  const savePlan = (plan) => write(PLAN_KEY, plan);
  const hasShop = () => !!read(SHOP_KEY);

  // ---------- Parts of the sample bench, by role ----------
  const LEGS = ["leg-bl", "leg-br", "leg-fl", "leg-fr"];
  const APRONS = ["apron-b", "apron-f", "apron-l", "apron-r"];
  const STRETCHERS = ["str-b", "str-f"];
  const FRAME = [...LEGS, ...APRONS, ...STRETCHERS];
  const HARDWOOD = [...FRAME, "top"];
  const ALL = [...HARDWOOD, "shelf"];

  // ---------- Steps and methods ----------
  // Method fields: tools [[id, qty]], min per unit, level 1–3 (difficulty), accuracy 1–3,
  // style hand | power | machine, ideal marks the method recommended with an ideal shop.
  const M = (id, name, tools, min, o = {}) => Object.assign({ id, name, tools: tools.map((t) => (Array.isArray(t) ? t : [t, 1])), min, level: 2, accuracy: 2, style: "power", how: [] }, o);

  const PHASES = [
    { id: "prep", name: "Prepare stock" },
    { id: "mark", name: "Mark and label" },
    { id: "join", name: "Cut the joinery" },
    { id: "shape", name: "Shape edges" },
    { id: "fit", name: "Dry fit and pre-sand" },
    { id: "glue", name: "Glue up" },
    { id: "finish", name: "Finish" },
  ];

  const STEPS = [
    {
      id: "s-check", phase: "prep", name: "Check materials and tools", qty: 1, unit: "check", parts: ALL,
      why: "Before you start, so that a missing board or clamp doesn't stop you halfway through a glue-up.",
      methods: [M("list", "Use the project lists", ["tape"], 15, { level: 1, accuracy: 3, style: "hand", ideal: true, how: [
        "Set aside the boards and offcuts listed above, and buy the rest before you start.",
        "Look for checks, knots near joints and cupping, and plan which board gives which part.",
        "Go through the tools for this project in the side panel and set aside anything you need to borrow or buy.",
      ] })],
    },
    {
      id: "s-mill", phase: "prep", name: "Mill the hardwood flat and square", qty: 11, unit: "parts", parts: HARDWOOD,
      why: "Every later step references a flat face and a square edge; joinery cut on twisted stock never closes.",
      methods: [
        M("machines", "Jointer, planer and table saw", ["jointer", "planer", "tablesaw"], 9, { level: 1, accuracy: 3, style: "machine", ideal: true, how: [
          "Rough cut every part 50 mm long and 5 mm wide.",
          "Flatten one face on the jointer, then joint an edge square to it.",
          "Plane the opposite face to 27 mm for oak and 25 mm for walnut, flipping the board between passes.",
          "Rip to width on the table saw with the jointed edge against the fence.",
        ] }),
        M("hand", "Hand planes", ["jackplane", "jointerplane", "mgauge", "square"], 35, { level: 3, accuracy: 2, style: "hand", how: [
          "Check each face for twist with winding sticks and plane the high corners first.",
          "Plane the face flat, then an edge square to it, and mark both as reference.",
          "Gauge the thickness all round from the reference face and plane down to the line.",
        ] }),
        M("s4s", "Buy surfaced lumber (S4S)", [], 2, { level: 1, accuracy: 2, style: "hand", note: "No milling, but about €35 more for the oak and walnut and less choice of thickness.", how: [
          "Ask the yard for oak surfaced to 27 mm and walnut to 25 mm.",
          "Let the boards acclimatise in the shop for a few days before cutting them.",
          "Check edges for straightness; surfaced boards still move after they are cut.",
        ] }),
      ],
    },
    {
      id: "s-gluetop", phase: "prep", name: "Edge-glue the walnut top", qty: 1, unit: "panel", parts: ["top"],
      why: "Early, because the glue needs a few hours before the panel can be flattened and cut to size.",
      methods: [
        M("jointer", "Jointed edges, one glue-up", ["jointer", ["barclamps", 4], "glue"], 50, { level: 1, accuracy: 3, style: "machine", ideal: true, how: [
          "Arrange the three boards for grain and colour and mark a triangle across them.",
          "Joint the mating edges, alternating face in and face out so small errors cancel.",
          "Glue and clamp with four bar clamps, alternating above and below the panel.",
        ] }),
        M("plane", "Hand-planed edges", ["jointerplane", ["barclamps", 4], "glue"], 80, { level: 3, accuracy: 3, style: "hand", how: [
          "Plane the mating edges as a pair, clamped together, so any angle cancels out.",
          "Check the joint against the light: it should close with hand pressure.",
          "Glue and clamp with four bar clamps.",
        ] }),
        M("track", "Track saw edges", ["circsaw", ["barclamps", 4], "glue"], 60, { level: 2, accuracy: 2, style: "power", how: [
          "Rip a straight edge on each board with the saw running along a straightedge.",
          "Dry clamp to check for gaps before gluing.",
          "Glue and clamp with four bar clamps.",
        ] }),
        M("twostage", "Two stages with two clamps", ["circsaw", ["barclamps", 2], "glue"], 110, { level: 2, accuracy: 2, style: "power", note: "Takes an extra evening, because the first joint must cure before the second.", how: [
          "Rip a straight edge on each board with a track saw.",
          "Glue the first two boards with two clamps and cauls, and let them cure for two hours.",
          "Joint the panel edge again and glue on the third board.",
        ] }),
      ],
    },
    {
      id: "s-cut", phase: "prep", name: "Cut parts to final length", qty: 12, unit: "parts", parts: ALL,
      why: "Identical parts are cut with the same setup, so legs and rails come out exactly the same length.",
      methods: [
        M("mitre", "Mitre saw with a stop block", ["mitersaw", "square"], 3, { level: 1, accuracy: 3, style: "machine", ideal: true, how: [
          "Square one end of each part.",
          "Clamp a stop block to the fence and cut the four legs, then the matching rails, without moving it.",
        ] }),
        M("sled", "Table saw crosscut sled", ["tablesaw", "square"], 4, { level: 1, accuracy: 3, style: "machine", how: [
          "Square one end on the sled.",
          "Set a stop on the sled fence and cut matching parts together.",
        ] }),
        M("circ", "Circular saw and square", ["circsaw", "square"], 6, { level: 2, accuracy: 2, style: "power", how: [
          "Mark all matching parts together, clamped side by side.",
          "Cut with the saw shoe against a clamped square.",
        ] }),
        M("hand", "Handsaw and shooting board", ["crosscut", "blockplane", "shootboard", "square"], 10, { level: 3, accuracy: 3, style: "hand", how: [
          "Knife a line all round and saw just outside it.",
          "Trim to the line on the shooting board, checking matching parts against each other.",
        ] }),
      ],
    },
    {
      id: "s-label", phase: "mark", name: "Label every part and its orientation", qty: 12, unit: "parts", parts: ALL, labels: true,
      why: "Before any joinery, because the marks tell you which face is the reference for every gauge line and where each part goes.",
      methods: [M("labels", "Part labels and triangle marks", ["labels", "square"], 2, { level: 1, accuracy: 3, style: "hand", ideal: true, how: [
        "Stand the parts in their final position and choose the best face of each to show outwards.",
        "Draw a cabinetmaker's triangle across each group, pointing up and to the front: legs, aprons, stretchers.",
        "Mark the reference face with a loop and the reference edge with a V that touches it.",
        "Stick the printed label on a hidden face. Its code matches the shop drawings, for example L1 is the back-left leg.",
        "Mark the joint numbers at every mortise and tenon pair, so each tenon goes back into the mortise it was fitted to.",
      ] })],
    },
    {
      id: "s-layout", phase: "mark", name: "Lay out the joinery", qty: 12, unit: "joints", parts: FRAME,
      why: "All joints are laid out from the reference faces before cutting, so the gauge setting never changes between parts.",
      methods: [
        M("mgauge2", "Mortise gauge", ["mortgauge", "knife", "square"], 4, { level: 2, accuracy: 3, style: "hand", ideal: true, how: [
          "Set the pins to your 6 mm chisel, not to a ruler.",
          "Gauge every mortise and every tenon from the reference face without resetting it.",
          "Knife the shoulder lines all round with the square.",
        ] }),
        M("mgauge", "Marking gauge set twice", ["mgauge", "knife", "square"], 6, { level: 2, accuracy: 2, style: "hand", how: [
          "Gauge the first cheek line on every part from the reference face.",
          "Reset the gauge 6 mm further and gauge the second line on every part.",
          "Knife the shoulder lines all round with the square.",
        ] }),
      ],
    },
    {
      id: "s-mortise", phase: "join", name: "Cut 12 mortises in the legs", qty: 12, unit: "mortises", parts: LEGS,
      why: "Mortises go first: it is far easier to fit a tenon to a finished mortise than the other way round.",
      methods: [
        M("mortiser", "Hollow chisel mortiser", ["mortiser"], 4, { level: 1, accuracy: 3, style: "machine", ideal: true, how: [
          "Fit the 6 mm chisel and set the fence so the chisel sits between the gauge lines.",
          "Set the depth stop to 32 mm, 2 mm deeper than the tenon.",
          "Cut the ends first, then the middle, leaving half-chisel steps so the chisel doesn't drift.",
        ] }),
        M("router", "Plunge router and edge guide", ["router", "straightbit", "edgeguide"], 8, { level: 2, accuracy: 3, style: "power", note: "Router mortises have round ends: round the tenon edges with a rasp or square the mortise with a chisel.", how: [
          "Clamp two legs side by side to give the router a wider base.",
          "Set the edge guide so the 6 mm bit is centred between the gauge lines.",
          "Plunge in 5 mm passes to 32 mm deep, stopping at the end marks.",
        ] }),
        M("drill", "Drill out and pare", ["drill", "forstner", "chisels", "mallet"], 16, { level: 2, accuracy: 2, style: "power", how: [
          "Drill a row of overlapping 6 mm holes to depth, using tape on the bit as a depth stop.",
          "Sight the drill against a square standing next to the leg to keep it plumb.",
          "Pare the walls back to the gauge lines with a wide chisel.",
        ] }),
        M("hand", "Mortise chisel and mallet", ["mortchisel", "mallet", "mortgauge"], 25, { level: 3, accuracy: 3, style: "hand", how: [
          "Clamp the leg over a solid part of the bench.",
          "Start 2 mm inside each end and chop towards the middle, levering out waste as you go.",
          "Chop the final 2 mm at each end once you are at depth.",
        ] }),
        M("domino", "Switch to loose tenons (Domino)", ["domino"], 3, { level: 1, accuracy: 3, style: "power", skips: ["s-tenon"], note: "Changes the joint to loose tenons: the rails get a mortise too and the tenon step disappears.", how: [
          "Set the joiner to the 6 × 40 cutter and 20 mm depth.",
          "Reference every cut from the top of the parts so the faces stay flush.",
          "Cut the mating mortises in the rail ends with the same settings.",
        ] }),
      ],
    },
    {
      id: "s-tenon", phase: "join", name: "Cut 12 tenons on the rails", qty: 12, unit: "tenons", parts: [...APRONS, ...STRETCHERS],
      why: "After the mortises, so each tenon can be trimmed to fit its own numbered mortise.",
      methods: [
        M("jig", "Table saw and tenoning jig", ["tablesaw", "tenonjig"], 6, { level: 2, accuracy: 3, style: "machine", ideal: true, how: [
          "Cut the shoulders first with the mitre gauge and a stop block.",
          "Cut the cheeks with the rail upright in the jig, sneaking up on the fit with a test piece.",
        ] }),
        M("dado", "Table saw and dado set", ["tablesaw", "dado"], 5, { level: 2, accuracy: 2, style: "machine", how: [
          "Set the dado stack 10 mm high and nibble each cheek with the rail flat on the saw.",
          "Flip the rail to cut the opposite cheek with the same setting.",
        ] }),
        M("bandsaw", "Bandsaw and shoulder plane", ["bandsaw", "shoulderplane"], 10, { level: 2, accuracy: 3, style: "machine", how: [
          "Saw the cheeks just outside the lines, then the shoulders.",
          "Trim each cheek with the shoulder plane until the tenon slides in with hand pressure.",
        ] }),
        M("handsaw", "Tenon saw and chisel", ["tenonsaw", "chisels", "square"], 20, { level: 3, accuracy: 2, style: "hand", how: [
          "Saw the cheeks on the waste side, tilting the rail to follow both lines.",
          "Saw the shoulders against a knife wall.",
          "Pare the cheeks to fit, testing in the numbered mortise as you go.",
        ] }),
        M("router", "Plunge router, freehand cheeks", ["router", "straightbit", "fclamps"], 9, { level: 2, accuracy: 2, style: "power", how: [
          "Clamp the four aprons together, ends flush, with a fence as a guide.",
          "Rout the cheeks of all four at once, setting the depth with a test piece.",
          "Repeat for the stretchers and round the tenon edges to match router mortises.",
        ] }),
      ],
    },
    {
      id: "s-groove", phase: "join", name: "Groove the stretchers for the shelf", qty: 2, unit: "grooves", parts: STRETCHERS,
      why: "Before glue-up, while the stretchers can still lie flat.",
      methods: [
        M("tablesaw", "Table saw, two passes", ["tablesaw"], 6, { level: 1, accuracy: 3, style: "machine", ideal: true, how: ["Set the blade 8 mm high and the fence 6 mm from it.", "Cut both stretchers, move the fence and widen the groove to 18 mm."] }),
        M("router", "Router and edge guide", ["router", "straightbit", "edgeguide"], 10, { level: 2, accuracy: 3, style: "power", how: ["Clamp the stretcher on edge and rout the groove in two 4 mm passes.", "Check the fit with the shelf offcut before the second stretcher."] }),
        M("plough", "Plough plane", ["ploughplane"], 20, { level: 3, accuracy: 2, style: "hand", how: ["Start the groove at the far end and walk backwards along the stretcher.", "Stop when the depth stop rides on the edge."] }),
      ],
    },
    {
      id: "s-fig8", phase: "join", name: "Recess the figure-8 fasteners", qty: 10, unit: "recesses", parts: APRONS,
      why: "On the loose aprons, because it is awkward to drill into the top edge of an assembled frame.",
      methods: [
        M("press", "Drill press and Forstner bit", ["drillpress", "forstner"], 3, { level: 1, accuracy: 3, style: "machine", ideal: true, how: ["Set the depth stop to the fastener thickness.", "Bore half on, half off the inside edge of each apron, every 250 mm."] }),
        M("drill", "Drill and Forstner bit", ["drill", "forstner"], 4, { level: 2, accuracy: 2, style: "power", how: ["Clamp a scrap block flush with the apron so the bit has support.", "Bore to depth, checking with the fastener."] }),
        M("chisel", "Chisel", ["chisels", "mallet"], 8, { level: 2, accuracy: 2, style: "hand", how: ["Trace the fastener and chop the recess with a narrow chisel."] }),
      ],
    },
    {
      id: "s-round", phase: "shape", name: "Round over the top edge", qty: 2.5, unit: "m", parts: ["top"],
      why: "Once the top is cut to size, and before final sanding.",
      methods: [
        M("router", "Router and R6 round-over bit", ["router", "roundover"], 5, { level: 1, accuracy: 3, style: "power", ideal: true, how: ["Rout the end grain first so the long edges clean up any breakout.", "Take a light second pass for a clean finish."] }),
        M("hand", "Block plane and rasp", ["blockplane", "rasp"], 14, { level: 2, accuracy: 2, style: "hand", how: ["Pencil guide lines 6 mm in from the corner on the face and the edge.", "Plane a chamfer to the lines, then the facets, and blend with a rasp."] }),
      ],
    },
    {
      id: "s-chamfer", phase: "shape", name: "Chamfer the leg feet", qty: 4, unit: "legs", parts: LEGS,
      why: "Stops the feet from splintering when the bench is dragged.",
      methods: [
        M("plane", "Block plane", ["blockplane"], 2, { level: 1, accuracy: 2, style: "hand", ideal: true, how: ["Plane a 3 mm chamfer round each foot, working towards the corners."] }),
        M("router", "Router and chamfer bit", ["router", "chamferbit"], 2, { level: 1, accuracy: 3, style: "power", how: ["Rout the chamfer with the leg clamped upright in the vice."] }),
      ],
    },
    {
      id: "s-dry", phase: "fit", name: "Dry fit the base", qty: 1, unit: "fit", parts: FRAME,
      why: "A rehearsal for the glue-up: you find tight tenons and the clamp order without glue setting.",
      methods: [M("clamps", "Clamped dry fit", [["barclamps", 2], "square"], 35, { level: 1, accuracy: 3, style: "hand", ideal: true, how: [
        "Assemble the two ends, then the long rails, in the order you will glue them.",
        "Check the diagonals: they should match within 1 mm.",
        "Trim tight tenons now and write the clamp positions on tape.",
      ] })],
    },
    {
      id: "s-presand", phase: "fit", name: "Sand the inside faces", qty: 1.2, unit: "m²", parts: [...FRAME, "shelf"],
      why: "Inside faces and corners are impossible to reach once the frame is glued.",
      methods: [
        M("ros", "Random orbital sander", ["ros"], 14, { level: 1, accuracy: 3, style: "power", ideal: true, how: ["Sand 120, then 180, keeping off the tenon cheeks and shoulders.", "Mark the sanded faces with chalk so you don't repeat them."] }),
        M("scraper", "Card scraper and sanding block", ["scraper"], 30, { level: 2, accuracy: 3, style: "hand", how: ["Scrape out tear-out first, then sand by hand along the grain."] }),
      ],
    },
    {
      id: "s-glueends", phase: "glue", name: "Glue the two leg frames", qty: 2, unit: "frames", parts: [...LEGS, "apron-l", "apron-r"],
      why: "Gluing the base in two stages keeps the open time short and each assembly easy to square.",
      methods: [M("clamps", "Two bar clamps per frame", [["barclamps", 2], "glue", "square"], 25, { level: 2, accuracy: 3, style: "hand", ideal: true, how: [
        "Brush glue in the mortises and on the tenon cheeks, not on the shoulders.",
        "Clamp across the legs at apron height and check that the frame is flat and square.",
        "Leave each frame clamped for one hour before the next glue-up.",
      ] })],
    },
    {
      id: "s-gluelong", phase: "glue", name: "Join the frames with the long rails", qty: 1, unit: "glue-up", parts: [...LEGS, "apron-b", "apron-f", "str-b", "str-f"],
      why: "Last structural glue-up, with the shelf slid into its grooves unglued so it can move.",
      methods: [
        M("clamps", "Four bar clamps", [["barclamps", 4], "glue", "square"], 30, { level: 2, accuracy: 3, style: "hand", ideal: true, how: [
          "Slide the shelf into the stretcher grooves before closing the frame.",
          "Clamp at apron and stretcher height on both sides.",
          "Measure the diagonals and push the long one shorter with a clamp if needed.",
        ] }),
        M("straps", "Ratchet straps", [["strap", 2], "glue", "square"], 35, { level: 2, accuracy: 2, style: "hand", note: "Cheap and fine for a frame this size; protect the corners with card.", how: [
          "Slide the shelf into the stretcher grooves.",
          "Wrap one strap at apron height and one at stretcher height and tighten them evenly.",
          "Check the diagonals and adjust before the glue grabs.",
        ] }),
      ],
    },
    {
      id: "s-shelf", phase: "glue", name: "Fix the shelf", qty: 1, unit: "shelf", parts: ["shelf"],
      why: "Only after the frame has cured, with one screw at each end so the plywood stays put.",
      methods: [M("screws", "Screws from below", ["drill"], 10, { level: 1, accuracy: 3, style: "power", ideal: true, how: ["Drill pilot holes through the stretchers into the shelf and drive four 4 × 30 screws."] })],
    },
    {
      id: "s-sand", phase: "finish", name: "Final sanding", qty: 2.05, unit: "m²", parts: ALL,
      why: "After assembly, to level the joints and remove pencil marks.",
      methods: [
        M("ros", "Random orbital sander", ["ros"], 10, { level: 1, accuracy: 3, style: "power", ideal: true, how: ["Sand 120, 180 and 220 on the top; 180 is enough for the base.", "Wipe with a damp cloth to raise the grain and give a last light pass."] }),
        M("scraper", "Card scraper and sanding block", ["scraper"], 24, { level: 2, accuracy: 3, style: "hand", how: ["Scrape the joint lines flush, then sand by hand along the grain."] }),
      ],
    },
    {
      id: "s-oil1", phase: "finish", name: "Hardwax oil, first coat", qty: 2.05, unit: "m²", parts: ALL, wait: 8,
      why: "Oil the top separately from the base so the underside is finished too and the top can't cup.",
      methods: [M("rag", "Rag on, wipe off", ["rags"], 9, { level: 1, accuracy: 3, style: "hand", ideal: true, how: ["Apply a thin coat with a rag, wait 15 minutes and wipe off all excess.", "Let it dry for 8 hours."] })],
    },
    {
      id: "s-oil2", phase: "finish", name: "Hardwax oil, second coat", qty: 2.05, unit: "m²", parts: ALL, wait: 24,
      why: "A second thin coat evens the sheen; the bench is ready after 24 hours of curing.",
      methods: [M("rag", "Rag on, wipe off", ["rags"], 7, { level: 1, accuracy: 3, style: "hand", ideal: true, how: ["Apply and wipe off as before, buff with a clean rag after an hour."] })],
    },
    {
      id: "s-top", phase: "finish", name: "Attach the top", qty: 1, unit: "top", parts: ["top"],
      why: "Last, after finishing, so the fasteners don't trap oil and the top can still move with the seasons.",
      methods: [M("fig8", "Figure-8 fasteners", ["drill"], 20, { level: 1, accuracy: 3, style: "power", ideal: true, how: ["Screw the fasteners into the apron recesses.", "Lay the top face down on a blanket, centre the base on it and screw up through the fasteners."] })],
    },
  ];
  const STEP = Object.fromEntries(STEPS.map((s) => [s.id, s]));

  // ---------- Plan logic ----------
  const have = (shop, id) => shop.owned[id] || 0;
  function missing(method, shop) {
    const out = [];
    for (const [id, n] of method.tools) {
      const got = have(shop, id);
      if (got < n) out.push({ id, need: n, got, buy: n - got, cost: (n - got) * (TOOL[id].price || 0) });
    }
    return out;
  }
  const feasible = (method, shop) => missing(method, shop).length === 0;
  const levelFactor = (shop) => (LEVELS.find((l) => l.id === shop.level) || LEVELS[1]).factor;

  // In "mine" mode the best method among those your workshop allows, nudged by how you
  // like to work; in "ideal" mode the method a well-equipped shop would use.
  function rank(method, shop) {
    const bonus = shop.style === "hand" ? (method.style === "hand" ? -40 : 0) : shop.style === "machine" ? (method.style === "machine" ? -10 : method.style === "hand" ? 10 : 0) : 0;
    return method.min + bonus - method.accuracy * 2;
  }
  function choose(step, shop, mode, picks) {
    const pick = step.methods.find((m) => m.id === picks[step.id]);
    if (mode === "ideal") return pick || step.methods.find((m) => m.ideal) || step.methods[0];
    if (pick && feasible(pick, shop)) return pick;
    const ok = step.methods.filter((m) => feasible(m, shop)).sort((a, b) => rank(a, shop) - rank(b, shop));
    return ok[0] || null;
  }
  const bestFeasible = (step, shop) => step.methods.filter((m) => feasible(m, shop)).sort((a, b) => rank(a, shop) - rank(b, shop))[0] || null;

  // The cheapest way to make a blocked step possible.
  function cheapestFix(step, shop) {
    return step.methods
      .map((m) => ({ method: m, missing: missing(m, shop) }))
      .map((x) => Object.assign(x, { cost: x.missing.reduce((a, b) => a + b.cost, 0) }))
      .sort((a, b) => a.cost - b.cost || a.method.min - b.method.min)[0];
  }

  function build(shop, mode, picks = {}) {
    const f = levelFactor(shop);
    const skipped = new Set();
    const rows = [];
    for (const step of STEPS) {
      const method = choose(step, shop, mode, picks);
      if (method && method.skips) method.skips.forEach((id) => skipped.add(id));
      rows.push({ step, method });
    }
    for (const row of rows) {
      row.skipped = skipped.has(row.step.id);
      row.blocked = !row.method && !row.skipped;
      row.minutes = row.skipped || !row.method ? 0 : Math.round(row.method.min * row.step.qty * f);
      row.missing = row.method ? missing(row.method, shop) : [];
    }
    const minutes = rows.reduce((a, r) => a + r.minutes, 0);
    const blocked = rows.filter((r) => r.blocked);
    // Tools the plan uses, with the steps that need them and the largest quantity.
    const tools = {};
    for (const r of rows) {
      if (r.skipped) continue;
      // A blocked step counts the tools of its cheapest fix, so the shopping list is complete.
      const m = r.method || cheapestFix(r.step, shop).method;
      for (const [id, n] of m.tools) {
        const t = (tools[id] = tools[id] || { id, need: 0, steps: [] });
        t.need = Math.max(t.need, n);
        t.steps.push(r.step.id);
      }
    }
    const toolList = Object.values(tools).map((t) => Object.assign(t, { got: have(shop, t.id), short: Math.max(0, t.need - have(shop, t.id)) }));
    const toBuy = toolList.filter((t) => t.short > 0).map((t) => Object.assign({}, t, { cost: t.short * (TOOL[t.id].price || 0) }));
    return { rows, minutes, blocked, tools: toolList, toBuy, cost: toBuy.reduce((a, t) => a + t.cost, 0), wait: rows.reduce((a, r) => a + (r.skipped ? 0 : r.step.wait || 0), 0) };
  }

  // Which single missing tools would save you the most time in "mine" mode.
  function upgrades(shop, picks = {}) {
    const base = build(shop, "mine", picks);
    const candidates = TOOLS.filter((t) => !have(shop, t.id) || t.qty);
    const out = [];
    const most = {};
    STEPS.forEach((s) => s.methods.forEach((m) => m.tools.forEach(([id, n]) => (most[id] = Math.max(most[id] || 0, n)))));
    for (const t of candidates) {
      if (!most[t.id] || have(shop, t.id) >= most[t.id]) continue;
      const add = most[t.id] - have(shop, t.id);
      const trial = Object.assign({}, shop, { owned: Object.assign({}, shop.owned, { [t.id]: have(shop, t.id) + add }) });
      const p = build(trial, "mine", picks);
      const saved = base.minutes - p.minutes;
      const unblocks = base.blocked.length - p.blocked.length;
      if (saved > 0 || unblocks > 0) out.push({ tool: t, add, saved, unblocks, cost: add * t.price });
    }
    return out.sort((a, b) => b.unblocks - a.unblocks || b.saved / (b.cost + 50) - a.saved / (a.cost + 50)).slice(0, 5);
  }

  function fmtH(min) {
    if (min < 60) return `${min} min`;
    const h = Math.floor(min / 60);
    const m = min % 60;
    return m ? `${h} h ${String(m).padStart(2, "0")}` : `${h} h`;
  }
  // Workshop screens are user-level and opened from several places; ?from= says where to return.
  const ORIGINS = {
    planning: { href: "a-planning.html", label: "Hall bench · Planning", done: "Open the Hall bench plan" },
    cutlist: { href: "a-workshop-flow.html#cutlist", label: "Hall bench · Cut list", done: "Back to the cut list" },
    start: { href: "a-start.html", label: "All projects", done: "Back to your projects" },
  };
  function origin() {
    const key = new URLSearchParams(location.search).get("from");
    const k = ORIGINS[key] ? key : "planning";
    return Object.assign({ key: k }, ORIGINS[k]);
  }
  const withFrom = (href, key = origin().key) => {
    const [path, hash] = href.split("#");
    return `${path}?from=${key}${hash ? "#" + hash : ""}`;
  };
  const fmtQty = (step) => `${step.qty} ${step.unit}`;
  const toolName = (id, n) => (n > 1 ? `${TOOL[id].name} × ${n}` : TOOL[id].name);

  window.BenchPlan = {
    CATS, TOOLS, TOOL, PRESETS, STYLES, LEVELS, PHASES, STEPS, STEP,
    loadShop, saveShop, loadPlan, savePlan, hasShop,
    have, missing, feasible, choose, bestFeasible, cheapestFix, build, upgrades, levelFactor,
    fmtH, fmtQty, toolName, origin, withFrom,
  };
})();
