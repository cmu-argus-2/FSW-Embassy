#[tokio::test]
    async fn driver() {
        // Configure expectations
        use embedded_hal_async::i2c::{ErrorKind, I2c, Operation};
        use embedded_hal_mock::eh1::i2c::{Mock as I2cMock, Transaction as I2cTransaction};
        use fsw_lib::drivers::adm1176::ADM1176 as adm1176;
        let expectations = [
            I2cTransaction::read(0x40, vec![0x80, 0x40, 0x00]),
        ];
        let mut i2c = I2cMock::new(&expectations);
        let mut sensor = adm1176::new(i2c, 0x40);
            match sensor.read_voltage_current().await {
                Ok((voltage, current)) => {
                    println!("voltage {}, current {} \n", voltage, current);
                }
                Err(e) => {
                    println!("{}", e);
                }
            }
        i2c = sensor.i2c;
        i2c.done();
        assert!(true);
    }