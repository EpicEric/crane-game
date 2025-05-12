extends Node3D

func _ready() -> void:
	$NetworkConnection.start()

#func _process(_delta: float) -> void:
	#if Input.is_action_just_pressed("debug_prize"):
		#$NetworkConnection.collect_prize("test")

func _on_network_connection_connection_url(url: String) -> void:
	$CanvasLayer/Label.text = url
	var image := QrCode.create(url)
	var texture := ImageTexture.create_from_image(image)
	$CanvasLayer/TextureRect.texture = texture
	$CanvasLayer/TextureRect.show()

func _on_network_connection_client_connected() -> void:
	$CanvasLayer/AnimationPlayer.play("fade_out")
	$CanvasLayer/TextureRect.hide()

func _on_network_connection_client_disconnected() -> void:
	$CanvasLayer/TextureRect.show()

func _on_prize_area_3d_body_entered(body: Node3D) -> void:
	if body.prize_name:
		$NetworkConnection.send_data(JSON.stringify({"type": "CollectPrize", "prize": body.prize_name}))
