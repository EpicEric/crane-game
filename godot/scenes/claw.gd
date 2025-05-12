extends CharacterBody3D


const SPEED = 3.0
var is_controllable := true
var is_closing := false
var is_returning_home := false
var initial_position = null

func _init():
	initial_position = position

func _physics_process(delta: float) -> void:
	if is_controllable:
		if Input.is_action_just_pressed("deploy_claw"):
			is_controllable = false
			is_closing = true
			$AnimationPlayer.play("crane_close")
			await $AnimationPlayer.animation_finished
			$Timer.start()
			await $Timer.timeout
			is_returning_home = true
			return
		#var input_dir := Input.get_vector("move_left", "move_right", "move_up", "move_down")
		var input_dir := Input.get_vector("move_up", "move_down", "move_right", "move_left")
		var direction := (transform.basis * Vector3(input_dir.x, 0, input_dir.y)).normalized()
		if direction:
			velocity.x = direction.x * SPEED
			velocity.z = direction.z * SPEED
		else:
			velocity.x = move_toward(velocity.x, 0, SPEED)
			velocity.z = move_toward(velocity.z, 0, SPEED)

		move_and_slide()
	
	elif is_returning_home:
		var step = SPEED * delta
		if abs(position.x - initial_position.x) < step and abs(position.z - initial_position.z) < step:
			position = initial_position
			is_returning_home = false
			is_closing = false
			$Timer.start()
			await $Timer.timeout
			$AnimationPlayer.play("crane_open")
			await $AnimationPlayer.animation_finished
			is_controllable = true
		else:
			var pos_change = Vector3(move_toward(position.x - initial_position.x, 0, step), 0, move_toward(position.z - initial_position.z, 0, step)).normalized() * step
			position.x = position.x - pos_change.x
			position.z = position.z - pos_change.z
