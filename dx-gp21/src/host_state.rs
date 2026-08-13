use dx_gp21_core::command::{BaudRate, ConstellationMask, UpdateRate};
use dx_gp21_core::types::*;
use dx_gp21_core::state::GnssStore;
use dx_gp21_core::nmea::{GgaData, RmcData, GsaData, GsvData, VtgData, ZdaData, DhvData, GstData};

#[derive(Default)]
pub struct GnssState {
    pub gga: Option<GgaData>,
    pub rmc: Option<RmcData>,
    pub vtg: Option<VtgData>,
    pub zda: Option<ZdaData>,
    pub gst: Option<GstData>,
    pub dhv: Option<DhvData>,
    pub fix_mode: FixMode,
    pub dop: DopValues,
    pub antenna: AntennaStatus,
    pub satellites: Vec<SatInfo>,
    /// Baud rate of the connected port.
    pub baud_rate: BaudRate,
    /// Positioning output rate.
    pub update_rate: UpdateRate,
    /// Active constellation selection.
    pub system_mask: ConstellationMask,
}

impl GnssStore for GnssState {
    fn update_gga(&mut self, d: GgaData) { self.gga = Some(d); }
    fn update_rmc(&mut self, d: RmcData) { self.rmc = Some(d); }
    fn update_vtg(&mut self, d: VtgData) { self.vtg = Some(d); }
    fn update_zda(&mut self, d: ZdaData) { self.zda = Some(d); }
    fn update_gst(&mut self, d: GstData) { self.gst = Some(d); }
    fn update_dhv(&mut self, d: DhvData) { self.dhv = Some(d); }
    fn update_antenna(&mut self, status: AntennaStatus) { self.antenna = status; }

    fn update_gsa(&mut self, d: GsaData) {
        self.fix_mode = d.fix_mode;
        self.dop = DopValues::from(d);
        for sat in self.satellites.iter_mut() {
            if sat.system == d.system && d.svids.contains(&Some(sat.svid)) {
                sat.used = true;
            }
        }
    }

    fn update_gsv(&mut self, d: GsvData) {
        // Empty GSV (total_in_view=0) is a "no sats for this signal band" report.
        // Don't clear sats from other bands of the same constellation.
        if d.total_in_view == 0 {
            return;
        }
        if d.msg_num == 1 {
            self.satellites.retain(|s| s.system != d.system);
        }
        for sat in d.sats.iter().flatten() {
            self.satellites.push(*sat);
        }
    }

    fn gga(&self) -> Option<&GgaData> { self.gga.as_ref() }
    fn rmc(&self) -> Option<&RmcData> { self.rmc.as_ref() }
    fn vtg(&self) -> Option<&VtgData> { self.vtg.as_ref() }
    fn zda(&self) -> Option<&ZdaData> { self.zda.as_ref() }
    fn gst(&self) -> Option<&GstData> { self.gst.as_ref() }
    fn dhv(&self) -> Option<&DhvData> { self.dhv.as_ref() }
    fn fix_mode(&self) -> FixMode { self.fix_mode }
    fn dop(&self) -> DopValues { self.dop }
    fn antenna(&self) -> AntennaStatus { self.antenna }
    fn satellites(&self) -> &[SatInfo] { &self.satellites }

    fn sats_used_count(&self) -> u8 {
        // GGA.sats_used is the authoritative count from the NMEA sentence itself
        // and avoids race conditions with the GSV/GSA update cycle.
        self.gga.map(|g| g.sats_used)
            .unwrap_or_else(|| self.satellites.iter().filter(|s| s.used).count() as u8)
    }

    fn sats_in_view_count(&self) -> u8 {
        self.satellites.len() as u8
    }
}
