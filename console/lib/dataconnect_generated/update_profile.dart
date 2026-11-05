part of 'generated.dart';

class UpdateProfileVariablesBuilder {
  String id;
  Optional<String> _name = Optional.optional(nativeFromJson, nativeToJson);

  final FirebaseDataConnect _dataConnect;  UpdateProfileVariablesBuilder name(String? t) {
   _name.value = t;
   return this;
  }

  UpdateProfileVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<UpdateProfileData> dataDeserializer = (dynamic json)  => UpdateProfileData.fromJson(jsonDecode(json));
  Serializer<UpdateProfileVariables> varsSerializer = (UpdateProfileVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<UpdateProfileData, UpdateProfileVariables>> execute() {
    return ref().execute();
  }

  MutationRef<UpdateProfileData, UpdateProfileVariables> ref() {
    UpdateProfileVariables vars= UpdateProfileVariables(id: id,name: _name,);
    return _dataConnect.mutation("UpdateProfile", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class UpdateProfileProfileUpdate {
  final String id;
  UpdateProfileProfileUpdate.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateProfileProfileUpdate otherTyped = other as UpdateProfileProfileUpdate;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  UpdateProfileProfileUpdate({
    required this.id,
  });
}

@immutable
class UpdateProfileData {
  final UpdateProfileProfileUpdate? profile_update;
  UpdateProfileData.fromJson(dynamic json):
  
  profile_update = json['profile_update'] == null ? null : UpdateProfileProfileUpdate.fromJson(json['profile_update']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateProfileData otherTyped = other as UpdateProfileData;
    return profile_update == otherTyped.profile_update;
    
  }
  @override
  int get hashCode => profile_update.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (profile_update != null) {
      json['profile_update'] = profile_update!.toJson();
    }
    return json;
  }

  UpdateProfileData({
    this.profile_update,
  });
}

@immutable
class UpdateProfileVariables {
  final String id;
  late final Optional<String>name;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  UpdateProfileVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']) {
  
  
  
    name = Optional.optional(nativeFromJson, nativeToJson);
    name.value = json['name'] == null ? null : nativeFromJson<String>(json['name']);
  
  }
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateProfileVariables otherTyped = other as UpdateProfileVariables;
    return id == otherTyped.id && 
    name == otherTyped.name;
    
  }
  @override
  int get hashCode => Object.hashAll([id.hashCode, name.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    if(name.state == OptionalState.set) {
      json['name'] = name.toJson();
    }
    return json;
  }

  UpdateProfileVariables({
    required this.id,
    required this.name,
  });
}

