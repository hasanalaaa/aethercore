import type { DriverDevice, DriverHub } from '../../lib/contracts';

export type ManualGroup = { vendor: string; devices: DriverDevice[]; hardwareIds: string[] };

// PCI vendor ids and the four-letter ACPI vendor ids that appear in `VEN_xxxx`.
const VENDORS: Record<string, string> = {
  '8086': 'Intel', '8087': 'Intel', INTC: 'Intel', '10DE': 'NVIDIA', '1002': 'AMD', '1022': 'AMD', AMDI: 'AMD',
  '10EC': 'Realtek', '14C3': 'MediaTek', '17CB': 'Qualcomm', '1B21': 'ASMedia',
};

// Support roots as plain text: shown and copied, never opened or fetched (the UI holds no remote address).
const SUPPORT: [RegExp, string][] = [
  [/asus/i, 'www.asus.com/support'], [/intel/i, 'www.intel.com/support'], [/dell/i, 'www.dell.com/support'],
  [/lenovo/i, 'support.lenovo.com'], [/hewlett|\bhp\b/i, 'support.hp.com'], [/micro-star|\bmsi\b/i, 'www.msi.com/support'],
  [/acer/i, 'www.acer.com/support'], [/gigabyte/i, 'www.gigabyte.com/support'], [/asrock/i, 'www.asrock.com/support'],
];

const GENERIC = /^(system product name|system manufacturer|to be filled by o\.e\.m\.|default string|not applicable|o\.e\.m\.|oem)$/i;

export function supportRoot(name: string): string {
  return SUPPORT.find(([pattern]) => pattern.test(name))?.[1] ?? '';
}

/** "ASUS PRIME Z790-P": the maker and the model, with firmware placeholders left out. Empty when nothing real is known. */
export function machineLabel(hub: Pick<DriverHub, 'machineManufacturer' | 'machineModel' | 'boardProduct'>): string {
  const real = (value: string) => (value.trim() && !GENERIC.test(value.trim()) ? value.trim() : '');
  return [real(hub.machineManufacturer), real(hub.machineModel) || real(hub.boardProduct)].filter(Boolean).join(' ');
}

function vendorOf(device: DriverDevice): string {
  for (const id of device.hardwareIds) {
    const hit = /VEN_([0-9A-Z]{4})/i.exec(id);
    const vendor = hit ? VENDORS[hit[1].toUpperCase()] : undefined;
    if (vendor) return vendor;
  }
  const maker = device.manufacturer.trim();
  return maker && !maker.startsWith('(') ? maker : '';
}

/** The vendor-and-device id alone (no subsystem or revision) is the one worth copying into a search. */
function generalId(device: DriverDevice): string {
  return device.hardwareIds.find((id) => /^[A-Z]+\\VEN_[0-9A-Z]{4}&DEV_[0-9A-Z]{4}$/i.test(id)) ?? device.hardwareIds[0] ?? '';
}

/** Devices with no driver and nothing AetherCore may install, grouped by vendor in the order first seen. */
export function manualRoutingGroups(devices: readonly DriverDevice[]): ManualGroup[] {
  const groups = new Map<string, ManualGroup>();
  for (const device of devices) {
    if (!device.missingDriver || device.candidates.some((candidate) => candidate.selectable)) continue;
    const vendor = vendorOf(device);
    const group = groups.get(vendor) ?? { vendor, devices: [], hardwareIds: [] };
    group.devices.push(device);
    const id = generalId(device);
    if (id && !group.hardwareIds.includes(id)) group.hardwareIds.push(id);
    groups.set(vendor, group);
  }
  return [...groups.values()];
}
