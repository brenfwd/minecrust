## Rust Enums/Results/Error handling

```rs
#[derive(Debug)]
enum PaymentMethod {
    Cash,
    CreditCard { credit_limit: f64, brand: String },
    DebitCard { brand: String },
    Chicken,
}

type PaymentResult = Result<String, String>;

fn accept_payment(payment: PaymentMethod, amount: f64) -> PaymentResult {
    match payment {
        PaymentMethod::Cash => {
            println!("Paid with cash");
            return Ok("N/A".to_string());
        }
        PaymentMethod::CreditCard {
            credit_limit,
            brand,
        } => {
            if credit_limit < amount {
                return Err("Insufficient funds".to_string());
            }
            println!("Paid with {} credit card", brand);
            return Ok("123ABC".to_string());
        }
        _ => {
            println!("Paid with some other method lol");
            return Ok("N/A".to_string());
        }
    }
}

fn main() -> Result<(), String> {
    let payment = PaymentMethod::CreditCard {
        credit_limit: 100.00,
        brand: "Chase".to_string(),
    };

    let payment_result = accept_payment(payment, 50.00);
    // println!("Transaction ID: {payment_result}");

    if let Err(ref errmsg) = payment_result {
        println!("Failed to process payment: {}", errmsg);
        return Err("Failed".to_string());
    }
    let payment_result = payment_result.unwrap();
    println!("Transaction ID: {payment_result}");

    // match payment_result {
    //     Ok(txid) => {
    //         println!("Transaction ID: {}", txid);
    //     }
    //     Err(errmsg) => {
    //         eprintln!("Failed to process payment: {}", errmsg);
    //     }
    // }

    Ok(())
}
```
