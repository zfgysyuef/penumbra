/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025 Shomy
*/
#[macro_use]
mod macros;
mod backend;
mod command;
pub mod port;

use std::time::Duration;

use log::{debug, error, info};

use crate::connection::command::Command;
use crate::connection::port::{ConnectionType, MTKPort};
use crate::error::{Error, Result};

/// Practical safety bound for a BROM SLA challenge or response.
///
/// Real Xiaomi challenges and RSA signatures are much smaller; the bound
/// prevents a malformed device response or pasted SIGN from causing an
/// unbounded allocation in an interactive authentication flow.
pub const BROM_SLA_MAX_DATA_SIZE: usize = 0x0FFF;

#[derive(Debug)]
pub struct Connection {
    pub port: Box<dyn MTKPort>,
    pub connection_type: ConnectionType,
    pub baudrate: u32,
}

impl Connection {
    pub fn new(port: Box<dyn MTKPort>) -> Self {
        let connection_type = port.get_connection_type();
        let baudrate = port.get_baudrate();

        Self { port, connection_type, baudrate }
    }

    // Writes the provided data to the device
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        self.port.write_all(data)
    }

    // Reads the exact number of bytes required to fill the provided buffer
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.port.read_exact(buf)
    }

    // Reads the specified number of bytes
    pub fn read_bytes(&mut self, size: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; size];
        self.port.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn read_u16_be(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.port.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    fn read_u16_le(&mut self) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.port.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    fn read_u32_be(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.port.read_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }

    pub fn check(&self, data: &[u8], expected_data: &[u8]) -> Result<()> {
        if data == expected_data {
            Ok(())
        } else {
            error!("Data mismatch. Expected: {:x?}, Got: {:x?}", expected_data, data);
            Err(Error::conn("Data mismatch"))
        }
    }

    pub fn echo(&mut self, data: &[u8], size: usize) -> Result<()> {
        self.write(data)?;
        let mut buf = vec![0u8; size];
        self.read(&mut buf)?;
        self.check(&buf, data)
    }

    /* BROM / Preloader download handlers below :D */

    pub fn handshake(&mut self) -> Result<()> {
        info!("Starting handshake...");
        self.port.set_timeout(Some(Duration::from_secs(3)))?;
        self.port.handshake()?;
        self.port.set_timeout(None)?;
        info!("Handshake completed!");
        Ok(())
    }

    pub fn jump_da(&mut self, address: u32) -> Result<()> {
        debug!("Jump to DA at 0x{:08X}", address);

        self.echo(&[Command::JumpDa as u8], 1)?;
        self.echo(&address.to_be_bytes(), 4)?;

        status_ok!(self);

        Ok(())
    }

    pub fn send_da(
        &mut self,
        da_data: &[u8],
        da_len: u32,
        address: u32,
        sig_len: u32,
    ) -> Result<()> {
        debug!("Sending DA, size: {}", da_data.len());
        self.echo(&[Command::SendDa as u8], 1)?;
        self.echo(&address.to_be_bytes(), 4)?;
        self.echo(&(da_len).to_be_bytes(), 4)?;
        self.echo(&sig_len.to_be_bytes(), 4)?;

        status_ok!(self);

        self.write(da_data)?;

        debug!("DA sent!");

        let checksum = self.read_u16_be()?;
        debug!("Received checksum: 0x{:04X}", checksum);

        status_ok!(self);

        Ok(())
    }

    pub fn send_auth(&mut self, data: &[u8]) -> Result<()> {
        self.echo(&[Command::SendAuth as u8], 1)?;

        let len = data.len() as u32;
        self.echo(&len.to_be_bytes(), 4)?;

        status_ok!(self);

        self.write(data)?;

        // Checksum
        self.read_u16_be()?;

        status_ok!(self);

        info!("Auth sent successfully!");
        Ok(())
    }

    /// Completes the Preloader/BROM one-time SLA challenge.
    ///
    /// The callback receives the exact BLOB consumed by Xiaomi's signer:
    /// 32-byte SoC ID followed by the challenge returned by command 0xE3,
    /// with each 16-bit word converted to host byte order. It must return the
    /// raw SIGN bytes; this method performs the inverse 16-bit conversion and
    /// sends them to the device.
    pub fn complete_brom_sla<F>(&mut self, soc_id: &[u8; 32], signer: &mut F) -> Result<()>
    where
        F: FnMut(&[u8]) -> Result<Vec<u8>> + ?Sized,
    {
        self.echo(&[Command::SlaChallenge as u8], 1)?;
        let status = self.read_u16_be()?;
        self.check_brom_sla_status("challenge request", status)?;

        let challenge_len = self.read_u32_be()? as usize;
        if challenge_len == 0 || challenge_len > BROM_SLA_MAX_DATA_SIZE {
            return Err(Error::proto(format!(
                "Invalid BROM SLA challenge length: {challenge_len}"
            )));
        }
        if !challenge_len.is_multiple_of(2) {
            return Err(Error::proto(format!(
                "BROM SLA challenge length must be even, got {challenge_len}"
            )));
        }

        let mut challenge = self.read_bytes(challenge_len)?;
        swap_u16_bytes(&mut challenge);

        let mut blob = Vec::with_capacity(soc_id.len() + challenge.len());
        blob.extend_from_slice(soc_id);
        blob.extend_from_slice(&challenge);

        let mut signature = signer(&blob)?;
        if signature.is_empty() || signature.len() > BROM_SLA_MAX_DATA_SIZE {
            return Err(Error::proto(format!(
                "Invalid BROM SLA signature length: {}",
                signature.len()
            )));
        }
        if !signature.len().is_multiple_of(2) {
            return Err(Error::proto(format!(
                "BROM SLA signature length must be even, got {}",
                signature.len()
            )));
        }

        let signature_len = signature.len() as u32;
        self.write(&signature_len.to_be_bytes())?;

        let echoed_len = self.read_u32_be()?;
        if echoed_len != signature_len {
            return Err(Error::proto(format!(
                "BROM SLA signature length mismatch: sent {signature_len}, device echoed {echoed_len}"
            )));
        }
        let status = self.read_u16_be()?;
        self.check_brom_sla_status("signature length", status)?;

        swap_u16_bytes(&mut signature);
        self.write(&signature)?;
        let status = self.read_u16_be()?;
        self.check_brom_sla_status("signature verification", status)?;

        info!("Preloader/BROM MI authentication completed successfully!");
        Ok(())
    }

    fn check_brom_sla_status(&self, phase: &str, status: u16) -> Result<()> {
        // This mirrors SP Flash Tool V6: status values below 0x1000 are
        // accepted, while MTK security errors live in the 0x1xxx/0x7xxx
        // ranges (for example 0x7017 and 0x7024).
        if status == 0x7017 {
            Err(Error::penumbra(
                "BROM SLA returned 0x7017 (tool auth is null); provide the matching --auth file and reconnect the device before requesting a new challenge",
            ))
        } else if status < 0x1000 {
            Ok(())
        } else {
            Err(Error::Status { ctx: format!("BROM SLA {phase} failed"), status: status.into() })
        }
    }

    pub fn get_hw_code(&mut self) -> Result<u16> {
        self.echo(&[Command::GetHwCode as u8], 1)?;

        let hw_code = self.read_u16_be()?;
        status_ok!(self);

        Ok(hw_code)
    }

    pub fn get_hw_sw_ver(&mut self) -> Result<(u16, u16, u16)> {
        self.echo(&[Command::GetHwSwVer as u8], 1)?;

        let hw_sub_code = self.read_u16_le()?;
        let hw_ver = self.read_u16_le()?;
        let sw_ver = self.read_u16_le()?;
        status_ok!(self);

        Ok((hw_sub_code, hw_ver, sw_ver))
    }

    pub fn get_soc_id(&mut self) -> Result<[u8; 32]> {
        let mut soc_id = [0u8; 32];

        self.echo(&[Command::GetSocId as u8], 1)?;

        let length = self.read_u32_be()? as usize;

        if length != soc_id.len() {
            return Err(Error::conn(format!(
                "Invalid SoC ID length: expected {}, got {length}",
                soc_id.len()
            )));
        }

        self.read(&mut soc_id)?;

        status_ok!(self);

        Ok(soc_id)
    }

    pub fn get_meid(&mut self) -> Result<[u8; 16]> {
        self.write(&[Command::GetMeId as u8])?;
        let mut echo = [0u8; 1];
        self.read(&mut echo)?;

        let mut meid = [0u8; 16];

        // IQO Preloader seems to have a custom security gate that blocks most commands
        // behind an OEM authentication challenge (0x90/0x91). Only a small whitelist of
        // commands (GET_HW_CODE, GET_HW_SW_VER, GET_SOC_ID, and the OEM commands) are
        // allowed before authentication. Blocked commands receive 0xDC instead of an echo.
        if echo[0] == 0xDC {
            return Err(Error::conn(
                "Command blocked by Preloader security. \
                This device requires OEM authentication before commands can be executed.",
            ));
        }

        if echo[0] != Command::GetMeId as u8 {
            return Err(Error::conn("Data mismatch"));
        }

        let length = self.read_u32_be()? as usize;

        if length > meid.len() {
            return Err(Error::conn("Invalid MEID length"));
        }

        self.read(&mut meid)?;

        status_ok!(self);

        Ok(meid)
    }

    /// Returns the target configuration of the device.
    /// This configuration can be interpreted as follows:
    ///
    /// SBC = target_config & 0x1
    /// SLA = target_config & 0x2
    /// DAA = target_config & 0x4
    pub fn get_target_config(&mut self) -> Result<u32> {
        self.echo(&[Command::GetTargetConfig as u8], 1)?;

        let config = self.read_u32_be()?;
        status_ok!(self);

        Ok(config)
    }

    pub fn get_pl_capabilities(&mut self) -> Result<u32> {
        self.echo(&[Command::GetPlCap as u8], 1)?;

        let cap0 = self.read_u32_be()?;
        let _cap1 = self.read_u32_be()?; // Reserved

        Ok(cap0)
    }

    /// Reads memory from the device with size, split into 4-byte chunks.
    pub fn read32(&mut self, address: u32, size: usize) -> Result<Vec<u8>> {
        let aligned = size.div_ceil(4) * 4;

        self.echo(&[Command::Read32 as u8], 1)?;
        self.echo(&address.to_be_bytes(), 4)?;
        self.echo(&((aligned / 4) as u32).to_be_bytes(), 4)?;

        status_ok!(self);

        let mut data = vec![0u8; aligned];
        for chunk in data.chunks_mut(4) {
            self.read(chunk)?;
        }

        status_ok!(self);

        data.truncate(size);
        Ok(data)
    }
}

fn swap_u16_bytes(data: &mut [u8]) {
    for word in data.chunks_exact_mut(2) {
        word.swap(0, 1);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::connection::port::{ConnectionType, MTKPort};

    #[derive(Debug)]
    struct MockPort {
        reads: VecDeque<u8>,
        writes: Arc<Mutex<Vec<u8>>>,
    }

    impl MockPort {
        fn new(reads: Vec<u8>, writes: Arc<Mutex<Vec<u8>>>) -> Self {
            Self { reads: reads.into(), writes }
        }
    }

    impl MTKPort for MockPort {
        fn open(&mut self) -> Result<()> {
            Ok(())
        }

        fn close(&mut self) -> Result<()> {
            Ok(())
        }

        fn read_exact(&mut self, buf: &mut [u8]) -> Result<usize> {
            if self.reads.len() < buf.len() {
                return Err(Error::io("mock read underflow"));
            }
            for byte in buf.iter_mut() {
                *byte = self.reads.pop_front().unwrap();
            }
            Ok(buf.len())
        }

        fn write_all(&mut self, buf: &[u8]) -> Result<()> {
            self.writes.lock().unwrap().extend_from_slice(buf);
            Ok(())
        }

        fn flush(&mut self) -> Result<()> {
            Ok(())
        }

        fn handshake(&mut self) -> Result<()> {
            Ok(())
        }

        fn get_connection_type(&self) -> ConnectionType {
            ConnectionType::Brom
        }

        fn get_baudrate(&self) -> u32 {
            0
        }

        fn get_port_name(&self) -> String {
            "mock".into()
        }

        fn set_timeout(&mut self, _timeout: Option<Duration>) -> Result<()> {
            Ok(())
        }

        fn find_device() -> Result<Option<Self>> {
            Ok(None)
        }

        fn ctrl_out(
            &mut self,
            _request_type: u8,
            _request: u8,
            _value: u16,
            _index: u16,
            _data: &[u8],
        ) -> Result<()> {
            Ok(())
        }

        fn ctrl_in(
            &mut self,
            _request_type: u8,
            _request: u8,
            _value: u16,
            _index: u16,
            _len: usize,
        ) -> Result<Vec<u8>> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn completes_brom_sla_using_spflash_v6_framing() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let reads = vec![
            0xE3, // command echo
            0x00, 0x00, // initial status
            0x00, 0x00, 0x00, 0x04, // challenge length (BE)
            0x11, 0x22, 0x33, 0x44, // challenge
            0x00, 0x00, 0x00, 0x04, // echoed SIGN length (BE)
            0x00, 0x00, // SIGN length status
            0x00, 0x00, // verification status
        ];
        let port = MockPort::new(reads, Arc::clone(&writes));
        let mut connection = Connection::new(Box::new(port));
        let soc_id = [0xAA; 32];

        connection
            .complete_brom_sla(&soc_id, &mut |blob| {
                assert_eq!(&blob[..32], &soc_id);
                assert_eq!(&blob[32..], &[0x22, 0x11, 0x44, 0x33]);
                Ok(vec![0x10, 0x20, 0x30, 0x40])
            })
            .unwrap();

        assert_eq!(
            *writes.lock().unwrap(),
            [
                0xE3, // command
                0x00, 0x00, 0x00, 0x04, // SIGN length (BE)
                0x20, 0x10, 0x40, 0x30, // swap16(SIGN)
            ]
        );
    }

    #[test]
    fn rejects_brom_sla_status_before_calling_signer() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let port = MockPort::new(vec![0xE3, 0x70, 0x17], Arc::clone(&writes));
        let mut connection = Connection::new(Box::new(port));
        let mut called = false;

        let error = connection
            .complete_brom_sla(&[0; 32], &mut |_| {
                called = true;
                Ok(vec![0; 0x100])
            })
            .unwrap_err();

        assert!(!called);
        assert!(error.to_string().contains("0x7017 (tool auth is null)"));
        assert_eq!(*writes.lock().unwrap(), [0xE3]);
    }

    #[test]
    fn rejects_non_32_byte_soc_id_without_consuming_more_input() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let port = MockPort::new(vec![0xE7, 0x00, 0x00, 0x00, 0x10], Arc::clone(&writes));
        let mut connection = Connection::new(Box::new(port));

        let error = connection.get_soc_id().unwrap_err();

        assert!(error.to_string().contains("expected 32, got 16"));
        assert_eq!(*writes.lock().unwrap(), [0xE7]);
    }
}
