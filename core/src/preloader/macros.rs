/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

macro_rules! status {
    ($self:ident, $expected:expr) => {{
        let status = $self.read_u16_be()?;
        if status != $expected {
            let err = crate::error::BrPlError::from_code(status);
            return Err(crate::error::Error::BrPl(err));
        }
    }};
}

macro_rules! status_ok {
    ($self:ident) => {{
        status!($self, 0);
    }};
}

#[allow(unused)]
macro_rules! status_any {
    ($self:ident, $($valid:expr),+ $(,)?) => {{
        let status = $self.read_u16_be()?;
        if ![$($valid),+].contains(&status) {
            let err = crate::error::BrPlError::from_code(status);
            return Err(crate::error::Error::BrPl(err));
        }
    }};
}
