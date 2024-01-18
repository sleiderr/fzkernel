use crate::{
    io::{inb, outb},
    wait_for_or,
};
use modular_bitfield::prelude::*;

pub mod kbd;

const PS2_STATUS_PORT: u16 = 0x64;
const PS2_DATA_PORT: u16 = 0x60;
const PS2_CMD_REG: u16 = 0x64;

#[derive(Debug)]
pub struct PS2Controller {
    port_status: PS2ControllerPortStatus,
}

#[derive(Debug)]
pub enum PS2ControllerPortStatus {
    DualPort,
    SinglePort,
    SecondPortOnly,
    Failure,
}

#[bitfield]
#[derive(Debug)]
pub struct PS2StatusRegister {
    output_buffer_status: bool,
    input_buffer_status: bool,
    system_flag: bool,
    cmd_flag: bool,
    #[skip]
    reserved1: bool,
    #[skip]
    reserved2: bool,
    timeout_err: bool,
    parity_err: bool,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct PS2ConfigurationByte {
    first_port_int: bool,
    second_port_int: bool,
    system_flag: bool,
    #[skip]
    reserved1: bool,
    first_port_clock_disabled: bool,
    second_port_clock_disabled: bool,
    first_port_translation: bool,
    #[skip]
    reserved2: bool,
}

impl PS2Controller {
    pub fn init() -> Option<Self> {
        let mut controller = PS2Controller {
            port_status: PS2ControllerPortStatus::Failure,
        };

        let mut conf = controller.read_configuration();

        conf.set_first_port_translation(false);
        conf.set_first_port_int(false);
        conf.set_second_port_int(false);
        conf.set_first_port_clock_disabled(false);
        conf.set_second_port_clock_disabled(false);

        controller.write_configuration(conf);

        if !controller.self_test() {
            return None;
        }

        controller.port_status = match (
            controller.first_port_self_test(),
            controller.second_port_self_test(),
        ) {
            (true, true) => {
                controller.send_command_polling(PS2ControllerCommand::EnableFirstPort);
                controller.send_command_polling(PS2ControllerCommand::EnableSecondPort);
                let mut conf = controller.read_configuration();

                conf.set_first_port_int(true);
                conf.set_second_port_int(true);

                controller.write_configuration(conf);
                controller.first_device_reset();
                controller.send_command_polling(PS2ControllerCommand::DisableSecondPort);

                PS2ControllerPortStatus::DualPort
            }
            (true, false) => {
                controller.send_command_polling(PS2ControllerCommand::EnableFirstPort);
                let mut conf = controller.read_configuration();

                conf.set_first_port_int(true);
                controller.write_configuration(conf);
                controller.first_device_reset();
                controller.send_command_polling(PS2ControllerCommand::DisableSecondPort);

                PS2ControllerPortStatus::SinglePort
            }
            (false, true) => {
                controller.send_command_polling(PS2ControllerCommand::EnableSecondPort);
                let mut conf = controller.read_configuration();

                conf.set_second_port_int(true);
                controller.write_configuration(conf);
                controller.second_device_reset();

                PS2ControllerPortStatus::SecondPortOnly
            }
            (false, false) => PS2ControllerPortStatus::Failure,
        };

        controller.first_device_identify();

        Some(controller)
    }

    fn first_device_identify(&self) {
        match self.port_status {
            PS2ControllerPortStatus::SinglePort | PS2ControllerPortStatus::DualPort => {
                self.send_first_port_polling(0xF5);
                let response: PS2DeviceResponse = self.read_polling().into();
                if !matches!(response, PS2DeviceResponse::Ack) {
                    return;
                }
                self.send_first_port_polling(0xF2);
                let response = self.read_polling().into();
                if !matches!(response, PS2DeviceResponse::Ack) {
                    return;
                }
                let mut response = [0u8; 2];
                response[0] = self.read_polling();
                response[1] = self.read_polling();
            }
            _ => (),
        }
    }

    fn second_device_identify(&self) {
        match self.port_status {
            PS2ControllerPortStatus::SecondPortOnly | PS2ControllerPortStatus::DualPort => {
                self.send_second_port_polling(0xF5);
                let response: PS2DeviceResponse = self.read_polling().into();
                if !matches!(response, PS2DeviceResponse::Ack) {
                    return;
                }
                self.send_second_port_polling(0xF2);
                let response = self.read_polling().into();
                if !matches!(response, PS2DeviceResponse::Ack) {
                    return;
                }
                let mut response = [0u8; 2];
                response[0] = self.read_polling();
                response[1] = self.read_polling();
            }
            _ => (),
        }
    }

    fn first_device_reset(&self) -> bool {
        self.send_first_port_polling(0xFF);

        let response = self.read_polling().into();

        matches!(response, PS2DeviceResponse::SelfTestPassed)
    }

    fn second_device_reset(&self) -> bool {
        self.send_second_port_polling(0xFF);

        let response = self.read_polling().into();

        matches!(response, PS2DeviceResponse::SelfTestPassed)
    }

    fn first_port_self_test(&self) -> bool {
        self.send_command_polling(PS2ControllerCommand::TestFirstPort);
        let result = self.read_polling();

        result == 0
    }

    fn second_port_self_test(&self) -> bool {
        self.send_command_polling(PS2ControllerCommand::TestSecondPort);
        let result = self.read_polling();

        result == 0
    }

    fn self_test(&self) -> bool {
        self.send_command_polling(PS2ControllerCommand::TestController);
        let result = self.read_polling();

        result == 0x55
    }

    fn send_first_port_polling(&self, data: u8) {
        wait_for_or!(!self.read_status().input_buffer_status(), 50, return);
        outb(PS2_DATA_PORT, data);
    }

    fn send_second_port_polling(&self, data: u8) {
        self.send_command_polling(PS2ControllerCommand::WriteSecondPortInput);
        wait_for_or!(!self.read_status().input_buffer_status(), 50, return);
        outb(PS2_DATA_PORT, data);
    }

    fn send_command_polling(&self, cmd: PS2ControllerCommand) {
        wait_for_or!(!self.read_status().input_buffer_status(), 50, return);
        outb(PS2_CMD_REG, cmd.into());
    }

    fn send_byte_polling(&self, data: u8) {
        wait_for_or!(!self.read_status().input_buffer_status(), 50, return);
        outb(PS2_CMD_REG, data);
    }

    fn read_polling(&self) -> u8 {
        wait_for_or!(self.read_status().output_buffer_status(), 50, return 0xff);
        inb(PS2_DATA_PORT)
    }

    pub fn read_status(&self) -> PS2StatusRegister {
        let status_byte = inb(PS2_STATUS_PORT);

        PS2StatusRegister::from_bytes([status_byte])
    }

    pub fn write_configuration(&self, conf: PS2ConfigurationByte) {
        self.send_command_polling(PS2ControllerCommand::WriteControllerConfiguration);
        self.send_byte_polling(conf.into_bytes()[0]);
    }

    pub fn read_configuration(&self) -> PS2ConfigurationByte {
        self.send_command_polling(PS2ControllerCommand::ReadControllerConfiguration);
        let conf_bytes = self.read_polling();

        PS2ConfigurationByte::from_bytes([conf_bytes])
    }
}

#[macro_export]
macro_rules! define_ps2_enum {
    ($enum: tt, $(($variant: tt, $cmd_code: literal)), *) => {
        #[derive(Debug)]
        pub enum $enum {
            $(
                $variant,
            )*
            Unknown
        }

        impl From<$enum> for u8 {
            fn from(value: $enum) -> Self {
                match value {
                    $(
                    $enum::$variant => $cmd_code,
                    )*
                    $enum::Unknown => 0xFF,
                }
            }
        }

        impl From<u8> for $enum {
            fn from(value: u8) -> Self {
                match value {
                    $(
                    $cmd_code => Self::$variant,
                        )*
                    _ => Self::Unknown
                }
            }
        }
    };
}

define_ps2_enum!(
    PS2ControllerCommand,
    (ReadControllerConfiguration, 0x20),
    (WriteControllerConfiguration, 0x60),
    (DisableSecondPort, 0xA7),
    (EnableSecondPort, 0xA8),
    (TestSecondPort, 0xA9),
    (TestController, 0xAA),
    (TestFirstPort, 0xAB),
    (DiagnosticDump, 0xAC),
    (DisableFirstPort, 0xAD),
    (EnableFirstPort, 0xAE),
    (ReadInputPort, 0xC0),
    (ReadOutputPort, 0xD0),
    (WriteOutputPort, 0xD1),
    (WriteFirstPortOutput, 0xD2),
    (WriteSecondPortOutput, 0xD3),
    (WriteSecondPortInput, 0xD4)
);

define_ps2_enum!(
    PS2DeviceResponse,
    (InternalError, 0x00),
    (SelfTestPassed, 0xAA),
    (EchoResponse, 0xEE),
    (Ack, 0xFA),
    (SelfTestFailed, 0xFC),
    (Resend, 0xFE),
    (AltInternalError, 0xFF)
);
