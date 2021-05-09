use futures::SinkExt;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use super::lines_in_codec::LinesInCodec;

const MOTD: &str = r#"
                      ____                                                     
  .--.--.           ,'  , `.                  ,---,      ,----..               
 /  /    '.      ,-+-,.' _ |         ,--,   .'  .' `\   /   /   \        ,---, 
|  :  /`. /   ,-+-. ;   , ||       ,'_ /| ,---.'     \ |   :     :      /_ ./| 
;  |  |--`   ,--.'|'   |  ;|  .--. |  | : |   |  .`\  |.   |  ;. /,---, |  ' : 
|  :  ;_    |   |  ,', |  ':,'_ /| :  . | :   : |  '  |.   ; /--`/___/ \.  : | 
 \  \    `. |   | /  | |  |||  ' | |  . . |   ' '  ;  :;   | ;  __.  \  \ ,' ' 
  `----.   \'   | :  | :  |,|  | ' |  | | '   | ;  .  ||   : |.' .'\  ;  `  ,' 
  __ \  \  |;   . |  ; |--' :  | | :  ' ; |   | :  |  '.   | '_.' : \  \    '  
 /  /`--'  /|   : |  | ,    |  ; ' |  | ' '   : | /  ; '   ; : \  |  '  \   |  
'--'.     / |   : '  |/     :  | : ;  ; | |   | '` ,/  '   | '/  .'   \  ;  ;  
  `--'---'  ;   | |`-'      '  :  `--'   \;   :  .'    |   :    /      :  \  \ 
            |   ;/          :  ,      .-./|   ,.'       \   \ .'        \  ' ; 
            '---'            `--`----'    '---'          `---`           `--`  
                                                                               
(c) 2021 smudgy.org

What is your login? "new" creates a new account.

Login: "#;

pub async fn send(
    connection: &mut Framed<TcpStream, LinesInCodec>,
) -> Result<(), super::lines_in_codec::LinesInCodecError> {
    connection.send(MOTD).await
}
